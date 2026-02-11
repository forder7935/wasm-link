(component
  (core module $m
    (func $burn (export "burn") (result i32)
      (local $i i32)
      (local.set $i (i32.const 1000))
      (block $done
        (loop $loop
          (local.set $i (i32.sub (local.get $i) (i32.const 1)))
          (br_if $done (i32.eqz (local.get $i)))
          (br $loop)
        )
      )
      (i32.const 42)
    )
  )
  (core instance $i (instantiate $m))
  (func $f (export "burn") (result u32) (canon lift (core func $i "burn")))
  (instance $inst (export "burn" (func $f)))
  (export "test:fuel/root" (instance $inst))
)
