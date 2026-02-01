(component
  (core module $m
    (func $test (export "test"))
  )
  (core instance $i (instantiate $m))
  (func $f (export "test") (canon lift (core func $i "test")))
  (instance $inst
    (export "test" (func $f))
  )
  (export "test:load-error/root" (instance $inst))
)
