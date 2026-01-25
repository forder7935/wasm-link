// use wasmtime::component::ResourceTable ;
// use wasmtime_wasi::{ WasiCtx, WasiCtxView, WasiView };

use super::PluginData ;



pub struct PluginContext<T: PluginData> {
    pub(super) data: T,
    // pub(super) wasi_ctx: WasiCtx,
    // pub(super) wasi_table: ResourceTable,
}

impl<T: PluginData> PluginContext<T> {
    pub fn new( data: T ) -> Self {
        Self {
            data,
            // wasi_ctx: WasiCtx::builder().build(),
            // wasi_table: ResourceTable::new(),
        }
    }
}

impl<T: PluginData + std::fmt::Debug> std::fmt::Debug for PluginContext<T> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct( "Plugin Instance" )
            .field( "data", &self.data )
            // .field( "wasi_ctx", &"WasiCtx" )
            // .field( "wasi_table", &"ResourceTable" )
            .finish()
    }
}

//  impl<T: PluginData> WasiView for PluginContext<T> {
//     fn ctx( &mut self ) -> wasmtime_wasi::WasiCtxView<'_> {
//         WasiCtxView { ctx: &mut self.wasi_ctx, table: &mut self.wasi_table }
//     }
// }
