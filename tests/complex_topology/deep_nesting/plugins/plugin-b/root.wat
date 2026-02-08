(component
  ;; Import level-c dependency
  (import "test:level-c/root" (instance $level_c
    (export "get-c" (func (result (result u32))))
  ))

  (alias export $level_c "get-c" (func $get_c))

  ;; Memory for lowering
  (core module $mem_module
    (memory (export "memory") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      i32.const 256
    )
  )
  (core instance $mem_inst (instantiate $mem_module))
  (alias core export $mem_inst "memory" (core memory $shared_mem))
  (alias core export $mem_inst "realloc" (core func $shared_realloc))

  (core func $lowered_get_c (canon lower (func $get_c) (memory $shared_mem) (realloc $shared_realloc)))
  (core instance $imports_c (export "get-c" (func $lowered_get_c)))
  (core instance $mem_imports (export "memory" (memory $shared_mem)))

  (core module $main_impl
    (import "level-c" "get-c" (func $get_c (param i32)))
    (import "mem" "memory" (memory 1))

    (func (export "get-b") (result i32)
      (call $get_c (i32.const 0))
      (i32.load (i32.const 4))
    )
  )

  (core instance $main_inst (instantiate $main_impl
    (with "level-c" (instance $imports_c))
    (with "mem" (instance $mem_imports))
  ))

  (alias core export $main_inst "get-b" (core func $core_get_b))
  (func $lifted_get_b (result u32) (canon lift (core func $core_get_b)))
  (instance $inst (export "get-b" (func $lifted_get_b)))
  (export "test:level-b/root" (instance $inst))
)
