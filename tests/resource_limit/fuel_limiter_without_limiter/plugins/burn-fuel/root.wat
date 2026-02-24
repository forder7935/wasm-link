(component
  (core module $m
    (func $burn (export "burn") (result i32)
      (i32.const 42)
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "burn") (result u32) (canon lift (core func $i "burn")))
  (instance $inst (export "burn" (func $f)))
  (export "test:fuel/root" (instance $inst))
)
