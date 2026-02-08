(component
  (core module $m
    (func $get_d (export "get-d") (result i32)
      i32.const 1
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "get-d") (result u32) (canon lift (core func $i "get-d")))
  (instance $inst
    (export "get-d" (func $f))
  )
  (export "test:interface-d/root" (instance $inst))
)
