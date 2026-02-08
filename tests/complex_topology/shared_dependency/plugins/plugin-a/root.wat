(component
  (import "test:interface-b/root" (instance $interface_b
    (export "get-b" (func (result (result u32))))
  ))
  (import "test:interface-c/root" (instance $interface_c
    (export "get-c" (func (result (result u32))))
  ))

  (alias export $interface_b "get-b" (func $get_b))
  (alias export $interface_c "get-c" (func $get_c))

  (core module $mem_module
    (memory (export "memory") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      i32.const 256
    )
  )
  (core instance $mem_inst (instantiate $mem_module))
  (alias core export $mem_inst "memory" (core memory $shared_mem))
  (alias core export $mem_inst "realloc" (core func $shared_realloc))

  (core func $lowered_get_b (canon lower (func $get_b) (memory $shared_mem) (realloc $shared_realloc)))
  (core func $lowered_get_c (canon lower (func $get_c) (memory $shared_mem) (realloc $shared_realloc)))
  (core instance $imports_b (export "get-b" (func $lowered_get_b)))
  (core instance $imports_c (export "get-c" (func $lowered_get_c)))
  (core instance $mem_imports (export "memory" (memory $shared_mem)))

  (core module $main_impl
    (import "interface-b" "get-b" (func $get_b (param i32)))
    (import "interface-c" "get-c" (func $get_c (param i32)))
    (import "mem" "memory" (memory 1))

    (func (export "get-value") (result i32)
      (call $get_b (i32.const 0))
      (call $get_c (i32.const 8))
      (i32.add
        (i32.load (i32.const 4))
        (i32.load (i32.const 12))
      )
    )
  )

  (core instance $main_inst (instantiate $main_impl
    (with "interface-b" (instance $imports_b))
    (with "interface-c" (instance $imports_c))
    (with "mem" (instance $mem_imports))
  ))

  (alias core export $main_inst "get-value" (core func $core_get_value))
  (func $lifted_get_value (result u32) (canon lift (core func $core_get_value)))
  (instance $inst (export "get-value" (func $lifted_get_value)))
  (export "test:shared/root" (instance $inst))
)
