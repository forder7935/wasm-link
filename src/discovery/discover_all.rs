use std::collections::HashMap ;
use itertools::Itertools ;

use crate::utils::Merge ;
use crate::utils::PartialSuccess ;
use crate::InterfaceId ;
use super::{ PluginData, InterfaceData };



pub fn discover_all<I, P, E>(
    plugins: Vec<P>,
    root_interface_id: InterfaceId,
) -> PartialSuccess<HashMap<InterfaceId, ( I, Vec<P> )>, E>
where
    I: InterfaceData,
    P: PluginData,
    E: From<I::Error> + From<P::Error>,
{

    let ( entries, plugin_errors ) = plugins.into_iter()
        .map(| handle | Result::<_, E>::Ok(( *handle.get_plug()?, handle )))
        .partition_result::<Vec<_>, Vec<_>, _, _>();

    let mut plugin_group_map = entries.into_iter().into_group_map();
    plugin_group_map.entry( root_interface_id ).or_default();

    let ( socket_map, interface_errors ) = plugin_group_map.into_iter()
        .map(|( id, plugins )| Ok(( id, ( I::new( id )?, plugins ))))
        .partition_result::<HashMap<_, _>, Vec<_>, _, _>();

    ( socket_map, plugin_errors.merge_all( interface_errors ) )

}
