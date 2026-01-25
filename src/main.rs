use wasm_compose::{ InterfaceId, PluginId, PluginTree, InterfaceData, PluginData, FunctionData, InterfaceCardinality, Engine, Component, Linker };



const _SOURCE_DIR: &str = "./appdata" ;
const ROOT_SOCKET_ID: InterfaceId = InterfaceId::new( 0x_00_00_00_00_u64 );
const ROOT_SOCKET_INTERFACE: &str = "root:startup/root" ;
const STARTUP_FUNCTION: &str = "startup" ;

fn main() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( plugin_tree, _ ) = PluginTree::<InterfaceDir, PluginDir>::new::<capnp::Error, _, _>( vec![], ROOT_SOCKET_ID );
    let ( root_socket, _ ) = plugin_tree.load( engine, &linker ).unwrap();

    let result = root_socket.dispatch_function_on_root( ROOT_SOCKET_INTERFACE, STARTUP_FUNCTION, false, &[] );
    println!( "{:#?}", result );

}

struct PluginDir {
    id: PluginId,
}

impl PluginDir {
    fn _new( id: PluginId ) -> Self { Self { id } }
}

impl PluginData for PluginDir {

    type Error = capnp::Error ;
    type SocketIter = Vec<InterfaceId> ;

    #[inline( always )] fn get_id( &self ) -> Result<&PluginId, capnp::Error> { Ok( &self.id )}
    #[inline( always )] fn get_plug( &self ) -> Result<&InterfaceId, Self::Error> { todo!() }
    #[inline( always )] fn get_sockets( &self ) -> Result<Self::SocketIter, Self::Error> { todo!() }

    fn component( &self, _engine: &Engine ) -> Result<Component, Self::Error> { todo!() }

}

struct InterfaceDir {
    _id: InterfaceId,
}

impl InterfaceData for InterfaceDir {

    type Error = capnp::Error ;
    type FunctionIter = Vec<FunctionData> ;
    type ResourceIter = Vec<String> ;

    fn new( id: InterfaceId ) -> Result<Self, Self::Error> { Ok( Self { _id: id })}
    
    #[inline( always )] fn get_package_name( &self ) -> Result<&str, Self::Error> { todo!() }
    #[inline( always )] fn get_cardinality( &self ) -> Result<InterfaceCardinality, Self::Error> { todo!() }
    #[inline( always )] fn get_functions( &self ) -> Result<Self::FunctionIter, Self::Error> { todo!() }
    #[inline( always )] fn get_resources( &self ) -> Result<Self::ResourceIter, Self::Error> { todo!() }

}
