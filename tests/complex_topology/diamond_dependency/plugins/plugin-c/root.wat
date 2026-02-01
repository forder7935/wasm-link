(component
  (core module $m
    (func $get_c (export "get-c") (result i32)
      i32.const 3
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "get-c") (result u32) (canon lift (core func $i "get-c")))
  (instance $inst
    (export "get-c" (func $f))
  )
  (export "test:iface-c/root" (instance $inst))
)
