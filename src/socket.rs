//! Runtime container for plugin instances or dispatch results.
//!
//! A [`Socket`] holds values (plugin instances or call results) in a shape that
//! matches a binding's [`Cardinality`]( crate::Cardinality ). This allows consumers to
//! handle results appropriately based on whether they expected one plugin or many.

use std::collections::HashMap ;
use std::sync::{ Mutex, MutexGuard, PoisonError };
use wasmtime::component::Val ;

use crate::plugin::PluginContext ;
use crate::plugin_instance::PluginInstance ;
use crate::DispatchError ;



/// Container for plugin instances or dispatch results.
///
/// The variant corresponds directly to the interface's [`Cardinality`]( crate::Cardinality ):
///
/// | Cardinality  | Socket Variant           | Contents                                     |
/// |--------------|--------------------------|----------------------------------------------|
/// | `ExactlyOne` | `ExactlyOne( T )`        | Single value, guaranteed present             |
/// | `AtMostOne`  | `AtMostOne( Option<T> )` | Optional single value                        |
/// | `AtLeastOne` | `AtLeastOne( HashMap )`  | Map of plugin ID → value, at least one entry |
/// | `Any`        | `Any( HashMap )`         | Map of plugin ID → value, may be empty       |
#[derive( Debug )]
pub enum Socket<T, Id> {
    /// Zero or one value. Used when cardinality is `AtMostOne`.
    AtMostOne( Option<T> ),
    /// Exactly one value, guaranteed present. Used when cardinality is `ExactlyOne`.
    ExactlyOne( T ),
    /// One or more values keyed by plugin ID. Used when cardinality is `AtLeastOne`.
    AtLeastOne( HashMap<Id, T> ),
    /// Zero or more values keyed by plugin ID. Used when cardinality is `Any`.
    Any( HashMap<Id, T> ),
}

impl<T, Id: Clone + std::hash::Hash + Eq> Socket<T, Id> {

    pub(crate) fn map<N>( &self, mut map: impl FnMut( &T ) -> N ) -> Socket<N, Id> {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some( t ) ) => Socket::AtMostOne( Some( map( t ))),
            Self::ExactlyOne( t ) => Socket::ExactlyOne( map( t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.iter().map(|( id, item ): ( &Id, _ )| ( id.clone(), map( item ) )).collect() ),
            Self::Any( vec ) => Socket::Any( vec.iter().map(|( id, item ): ( &Id, _ )| ( id.clone(), map( item ) )).collect() ),
        }
    }

    pub(crate) fn map_mut<N>( self, mut map: impl FnMut(T) -> N ) -> Socket<N, Id> {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some( t )) => Socket::AtMostOne( Some( map( t ))),
            Self::ExactlyOne( t ) => Socket::ExactlyOne( map( t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.into_iter().map(|( id, item )| ( id, map( item ) )).collect() ),
            Self::Any( vec ) => Socket::Any( vec.into_iter().map(|( id, item )| ( id, map( item ))).collect() ),
        }
    }
}

impl<PluginId, Ctx> Socket<Mutex<PluginInstance<PluginId, Ctx>>, PluginId>
where
    PluginId: Clone + std::hash::Hash + Eq,
    Ctx: PluginContext,
{

    #[allow( clippy::type_complexity )]
    pub(crate) fn get( &self, id: &PluginId ) -> Result<
        Option<&Mutex<PluginInstance<PluginId, Ctx>>>,
        PoisonError<MutexGuard<'_, PluginInstance<PluginId, Ctx>>>
    > {
        Ok( match self {
            Self::AtMostOne( Option::None ) => None,
            Self::AtMostOne( Some( plugin )) | Self::ExactlyOne( plugin ) => {
                if &plugin.lock()?.id == id { Some( plugin ) } else { None }
            },
            Self::AtLeastOne( plugins ) | Self::Any( plugins ) => plugins.get( id ),
        })
    }

    pub(crate) fn dispatch_function(
        &self,
        interface_path: &str,
        function: &str,
        has_return: bool,
        data: &[Val],
    ) -> Socket<Result<Val, DispatchError>, PluginId> {
        self.map(| plugin | plugin
            .lock().map_err(|_| DispatchError::LockRejected )
            .and_then(| mut lock | lock.dispatch( interface_path, function, has_return, data ))
        )
    }
}

impl<Id> From<Socket<Val, Id>> for Val {
    fn from( socket: Socket<Val, Id> ) -> Self {
        match socket {
            Socket::AtMostOne( Option::None ) => Val::Option( Option::None ),
            Socket::AtMostOne( Some( val )) => Val::Option( Some( Box::new( val ))),
            Socket::ExactlyOne( val ) => val,
            Socket::AtLeastOne( items )
            | Socket::Any( items ) => Val::List( items.into_values().collect() ),
        }
    }
}
