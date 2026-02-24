(component
  (core module $m
    (memory 1)
    (func $grow_memory (export "grow-memory") (result i32)
      (memory.grow (i32.const 1))
    )
  )
  (core instance $i (instantiate $m))
  (func $f (result s32) (canon lift (core func $i "grow-memory")))
  (instance $inst (export "grow-memory" (func $f)))
  (export "test:memory/root" (instance $inst))
)
