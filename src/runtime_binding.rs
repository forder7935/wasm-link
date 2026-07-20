macro_rules! define_runtime_bindings {
	( $instance:ident, $binding_doc:literal, $binding_any_doc:literal ) => {
		type PluginSockets<Id, Ctx, Plugins> =
			<Plugins as crate::cardinality::Cardinality<Id, $instance<Ctx>>>::Rebind<
				std::sync::Arc<futures::lock::Mutex<$instance<Ctx>>>,
			>;
		type Results<Id, Ctx, Plugins> =
			<PluginSockets<Id, Ctx, Plugins> as crate::cardinality::Cardinality<
				Id,
				std::sync::Arc<futures::lock::Mutex<$instance<Ctx>>>,
			>>::Rebind<Result<wasmtime::component::Val, crate::DispatchError>>;

		#[doc = $binding_doc]
		pub struct Binding<Id, Ctx, Plugins = crate::cardinality::ExactlyOne<Id, $instance<Ctx>>>(
			crate::binding::Binding<Id, Ctx, Plugins, $instance<Ctx>>,
		)
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
			Plugins: crate::cardinality::Cardinality<Id, $instance<Ctx>> + 'static,
			PluginSockets<Id, Ctx, Plugins>: Send + Sync;

		impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
			Plugins: crate::cardinality::Cardinality<Id, $instance<Ctx>> + 'static,
			PluginSockets<Id, Ctx, Plugins>: crate::cardinality::Cardinality<
				Id,
				std::sync::Arc<futures::lock::Mutex<$instance<Ctx>>>,
			> + Send + Sync,
		{
			/// Creates a binding.
			pub fn new(
				package_name: impl Into<String>,
				interfaces: std::collections::HashMap<String, Interface>,
				plugins: Plugins,
			) -> Self {
				Self( crate::binding::Binding::new(
					package_name,
					interfaces.into_iter()
						.map(|( name, interface )| ( name, interface.into_metadata() ))
						.collect(),
					plugins,
				))
			}
		}

		impl<Id, Ctx, Plugins> Clone for Binding<Id, Ctx, Plugins>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
			Plugins: crate::cardinality::Cardinality<Id, $instance<Ctx>> + 'static,
			PluginSockets<Id, Ctx, Plugins>: Send + Sync,
		{
			fn clone( &self ) -> Self { Self( self.0.clone() ) }
		}

		impl<Id, Ctx, Plugins> std::fmt::Debug for Binding<Id, Ctx, Plugins>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
			Ctx: crate::PluginContext + std::fmt::Debug + 'static,
			Plugins: crate::cardinality::Cardinality<Id, $instance<Ctx>> + 'static,
			PluginSockets<Id, Ctx, Plugins>: Send + Sync + std::fmt::Debug,
		{
			fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
				self.0.fmt( formatter )
			}
		}

		#[doc = $binding_any_doc]
		#[derive( Debug )]
		pub struct BindingAny<Id, Ctx>( crate::binding::BindingAny<Id, Ctx, $instance<Ctx>> )
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static;

		impl<Id, Ctx> Clone for BindingAny<Id, Ctx>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
		{
			fn clone( &self ) -> Self { Self( self.0.clone() ) }
		}

		impl<Id, Ctx> BindingAny<Id, Ctx>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
		{
			fn into_core( self ) -> crate::binding::BindingAny<Id, Ctx, $instance<Ctx>> {
				self.0
			}
		}

		macro_rules! binding_from {
			( $cardinality:ident ) => {
				impl<Id, Ctx> From<Binding<Id, Ctx, crate::cardinality::$cardinality<Id, $instance<Ctx>>>>
					for BindingAny<Id, Ctx>
				where
					Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
					Ctx: crate::PluginContext + 'static,
				{
					fn from(
						binding: Binding<Id, Ctx, crate::cardinality::$cardinality<Id, $instance<Ctx>>>,
					) -> Self {
						Self( binding.0.into() )
					}
				}
			};
		}

		binding_from!( ExactlyOne );
		binding_from!( AtMostOne );
		binding_from!( AtLeastOne );
		binding_from!( Any );

		impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: crate::PluginContext + 'static,
			Plugins: crate::cardinality::Cardinality<Id, $instance<Ctx>> + 'static,
			PluginSockets<Id, Ctx, Plugins>: Send + Sync,
			BindingAny<Id, Ctx>: From<Self>,
		{
			/// Erases this binding's cardinality.
			pub fn into_any( self ) -> BindingAny<Id, Ctx> { self.into() }
		}
	};
}

pub(crate) use define_runtime_bindings;
