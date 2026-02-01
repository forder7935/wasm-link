(component
  (core module $m
    (func $func_b (export "func-b"))
  )
  (core instance $i (instantiate $m))
  (func $f (export "func-b") (canon lift (core func $i "func-b")))
  (instance $inst
    (export "func-b" (func $f))
  )
  (export "test:interface-b/root" (instance $inst))
)
