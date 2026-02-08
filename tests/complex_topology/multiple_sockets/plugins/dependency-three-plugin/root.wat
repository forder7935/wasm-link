(component
  (core module $m
    (func $get_three (export "get-three") (result i32)
      i32.const 3
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "get-three") (result u32) (canon lift (core func $i "get-three")))
  (instance $inst
    (export "get-three" (func $f))
  )
  (export "test:dependency-three/root" (instance $inst))
)
