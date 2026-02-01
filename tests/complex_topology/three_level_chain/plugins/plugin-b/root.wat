(component
  (core module $m
    (func $get_b (export "get-b") (result i32)
      i32.const 200
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "get-b") (result u32) (canon lift (core func $i "get-b")))
  (instance $inst
    (export "get-b" (func $f))
  )
  (export "test:level-b/root" (instance $inst))
)
