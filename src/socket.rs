use std::collections::HashMap ;
use std::sync::Mutex ;
use wasmtime::component::Val ;
use nonempty_collections::{ NEMap, NonEmptyIterator, IntoNonEmptyIterator };

use crate::plugin::PluginContext ;
use crate::plugin_instance::PluginInstance ;
use crate::{ DispatchError, Function };



/// Runtime container for plugin instances or dispatch results.
///
/// A [`Socket`] holds values (plugin instances or call results) in a shape that
/// encodes cardinality constraints directly in the type system. The enum variant
/// determines how many values are present, allowing consumers to handle results
/// appropriately based on whether they expected one plugin or many.
#[derive( Debug )]
pub enum Socket<T, Id> {
    /// Zero or one value with ID. Used when cardinality is `AtMostOne`.
    AtMostOne( Option<( Id, T )> ),
    /// Exactly one value with ID, guaranteed present. Used when cardinality is `ExactlyOne`.
    ExactlyOne( Id, T ),
    /// One or more values keyed by plugin ID. Used when cardinality is `AtLeastOne`.
    AtLeastOne( NEMap<Id, T> ),
    /// Zero or more values keyed by plugin ID. Used when cardinality is `Any`.
    Any( HashMap<Id, T> ),
}

impl<T, Id: std::hash::Hash + Eq> Socket<T, Id> {

    pub(crate) fn map<N>( &self, mut map: impl FnMut( &Id, &T ) -> N ) -> Socket<N, Id> where Id: Clone {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some(( id, t ))) => Socket::AtMostOne( Some(( id.clone(), map( id, t )))),
            Self::ExactlyOne( id, t ) => Socket::ExactlyOne( id.clone(), map( id, t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.nonempty_iter().map(|( id, item )| ( id.clone(), map( id, item ))).collect() ),
            Self::Any( vec ) => Socket::Any( vec.iter().map(|( id, item )| ( id.clone(), map( id, item ))).collect() ),
        }
    }

    pub(crate) fn map_mut<N>( self, mut map: impl FnMut(T) -> N ) -> Socket<N, Id> {
        match self {
            Self::AtMostOne( Option::None ) => Socket::AtMostOne( Option::None ),
            Self::AtMostOne( Some(( id, t ))) => Socket::AtMostOne( Some(( id, map( t )))),
            Self::ExactlyOne( id, t ) => Socket::ExactlyOne( id, map( t )),
            Self::AtLeastOne( vec ) => Socket::AtLeastOne( vec.into_nonempty_iter().map(|( id, item )| ( id, map( item ))).collect() ),
            Self::Any( vec ) => Socket::Any( vec.into_iter().map(|( id, item )| ( id, map( item ))).collect() ),
        }
    }
}

impl<PluginId, Ctx> Socket<Mutex<PluginInstance<Ctx>>, PluginId>
where
    PluginId: Clone + std::hash::Hash + Eq,
    Ctx: PluginContext,
{

    /// Note, if cardinality is `AtMostOne` or `ExactlyOne`, the id is ignored
    pub(crate) fn get( &self, id: &PluginId ) -> Option<&Mutex<PluginInstance<Ctx>>> {
        match self {
            Self::AtMostOne( Option::None ) => None,
            Self::AtMostOne( Some(( _, plugin ))) | Self::ExactlyOne( _, plugin ) => Some( plugin ),
            Self::AtLeastOne( plugins ) => plugins.get( id ),
            Self::Any( plugins ) => plugins.get( id ),
        }
    }

    pub(crate) fn dispatch_function(
        &self,
        interface_path: &str,
        function_name: &str,
        function: &Function,
        data: &[Val],
    ) -> Socket<Result<Val, DispatchError>, PluginId> {
        self.map(| _, plugin | plugin
            .lock().map_err(|_| DispatchError::LockRejected )
            .and_then(| mut lock | lock.dispatch(
                interface_path,
                function_name,
                function,
                data,
            ))
        )
    }
}

impl<Id: std::hash::Hash + Eq + Into<Val>> From<Socket<Val, Id>> for Val {
    fn from( socket: Socket<Val, Id> ) -> Self {
        match socket {
            Socket::AtMostOne( Option::None ) => Val::Option( Option::None ),
            Socket::AtMostOne( Some(( _, val ))) => Val::Option( Some( Box::new( val ))),
            Socket::ExactlyOne( _, val ) => val,
            Socket::AtLeastOne( items ) => Val::List(
                items.into_iter()
                    .map(|( id, val )| Val::Tuple( vec![ id.into(), val ]))
                    .collect()
            ),
            Socket::Any( items ) => Val::List(
                items.into_iter()
                    .map(|( id, val )| Val::Tuple( vec![ id.into(), val ]))
                    .collect()
            ),
        }
    }
}
