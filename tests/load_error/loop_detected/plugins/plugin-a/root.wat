(component
  (core module $m
    (func $func_a (export "func-a"))
  )
  (core instance $i (instantiate $m))
  (func $f (export "func-a") (canon lift (core func $i "func-a")))
  (instance $inst
    (export "func-a" (func $f))
  )
  (export "test:interface-a/root" (instance $inst))
)
