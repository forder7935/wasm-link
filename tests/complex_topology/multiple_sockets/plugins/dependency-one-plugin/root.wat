(component
  (core module $m
    (func $get_one (export "get-one") (result i32)
      i32.const 1
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "get-one") (result u32) (canon lift (core func $i "get-one")))
  (instance $inst
    (export "get-one" (func $f))
  )
  (export "test:dependency-one/root" (instance $inst))
)
