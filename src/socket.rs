use std::collections::HashMap ;
use std::sync::{ RwLock, RwLockReadGuard, PoisonError };
use wasmtime::component::Val ;

use crate::plugin::{ PluginId, PluginData };
use crate::plugin_instance::PluginInstance ;
use crate::loading::DispatchError ;



/// Container for plugin instances matching an interface's cardinality.
///
/// The variant used reflects the interface's cardinality constraint:
/// - `AtMostOne` - Zero or one plugin allowed
/// - `ExactlyOne` - Exactly one plugin required
/// - `AtLeastOne` - One or more plugins required
/// - `Any` - Zero or more plugins allowed
#[derive( Debug )]
pub enum Socket<T> {
    AtMostOne( Option<T> ),
    ExactlyOne( T ),
    AtLeastOne( HashMap<PluginId, T> ),
    Any( HashMap<PluginId, T> ),
}

impl<T> Socket<T> {

    pub(crate) fn map<N>( &self, mut map: impl FnMut( &T ) -> N ) -> Socket<N> {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some( t )) => Socket::AtMostOne( Some( map( t ))),
            Self::ExactlyOne( t ) => Socket::ExactlyOne( map( t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.iter().map(|( id, item ): ( &PluginId, _ )| ( *id, map( item ) )).collect() ),
            Self::Any( vec ) => Socket::Any( vec.iter().map(|( id, item ): ( &PluginId, _ )| ( *id, map( item ) )).collect() ),
        }
    }

    pub(crate) fn map_mut<N>( self, mut map: impl FnMut(T) -> N ) -> Socket<N> {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some( t )) => Socket::AtMostOne( Some( map( t ))),
            Self::ExactlyOne( t ) => Socket::ExactlyOne( map( t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.into_iter().map(|( id, item )| ( id, map( item ) )).collect() ),
            Self::Any( vec ) => Socket::Any( vec.into_iter().map(|( id, item )| ( id, map( item ))).collect() ),
        }
    }
}

impl<T: PluginData> Socket<RwLock<PluginInstance<T>>> {

    #[allow( clippy::type_complexity )]
    pub(crate) fn get( &self, id: PluginId ) -> Result<Option<&RwLock<PluginInstance<T>>>,PoisonError<RwLockReadGuard<'_, PluginInstance<T>>>> {
        Ok( match self {
            Self::AtMostOne( Option::None ) => None,
            Self::AtMostOne( Some( plugin )) | Self::ExactlyOne( plugin ) => {
                if plugin.read()?.id == id { Some( plugin ) } else { None }
            },
            Self::AtLeastOne( plugins ) | Self::Any( plugins ) => plugins.get( &id ),
        })
    }

    pub(crate) fn dispatch_function<IE: std::error::Error>(
        &self,
        interface_path: &str,
        function: &str,
        has_return: bool,
        data: &[Val],
    ) -> Socket<Result<Val, DispatchError<IE>>> {
        self.map(| plugin | plugin
            .write().map_err(|_| DispatchError::Deadlock )
            .and_then(| mut lock | lock.dispatch( interface_path, function, has_return, data ))
        )
    }
}

impl From<Socket<Val>> for Val {
    fn from( socket: Socket<Val> ) -> Self {
        match socket {
            Socket::AtMostOne( Option::None ) => Val::Option( Option::None ),
            Socket::AtMostOne( Some( val )) => Val::Option( Some( Box::new( val ))),
            Socket::ExactlyOne( val ) => val,
            Socket::AtLeastOne( items )
            | Socket::Any( items ) => Val::List( items.into_values().collect() ),
        }
    }
}
