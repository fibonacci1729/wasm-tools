(component
  (core module (;0;)
    (type (;0;) (func (param i32)))
    (type (;1;) (func (param i32) (result i32)))
    (type (;2;) (func (result i32)))
    (import "[export]foo:bar/a" "[resource-drop-own]r" (func (;0;) (type 0)))
    (import "[export]foo:bar/a" "[resource-rep]r" (func (;1;) (type 1)))
    (func (;2;) (type 2) (result i32)
      unreachable
    )
    (export "some-name#f" (func 2))
    (@producers
      (processed-by "wit-component" "$CARGO_PKG_VERSION")
      (processed-by "my-fake-bindgen" "123.45")
    )
  )
  (type (;0;) (resource (rep i32)))
  (type (;1;) (own 0))
  (core func (;0;) (canon resource.drop 1))
  (core func (;1;) (canon resource.rep 0))
  (core instance (;0;)
    (export "[resource-drop-own]r" (func 0))
    (export "[resource-rep]r" (func 1))
  )
  (core instance (;1;) (instantiate 0
      (with "[export]foo:bar/a" (instance 0))
    )
  )
  (component (;0;)
    (import "import-type-r" (type (;0;) (sub resource)))
    (export (;1;) "r" (type 0))
  )
  (instance (;0;) (instantiate 0
      (with "import-type-r" (type 0))
    )
  )
  (export (;1;) (interface "foo:bar/a") (instance 0))
  (alias export 1 "r" (type (;2;)))
  (component (;1;)
    (import "import-type-r" (type (;0;) (sub resource)))
    (export (;1;) "r" (type 0))
  )
  (instance (;2;) (instantiate 1
      (with "import-type-r" (type 2))
    )
  )
  (export (;3;) (interface "foo:bar/b") (instance 2))
  (alias export 3 "r" (type (;3;)))
  (type (;4;) (own 3))
  (type (;5;) (func (result 4)))
  (alias core export 1 "some-name#f" (core func (;2;)))
  (func (;0;) (type 5) (canon lift (core func 2)))
  (component (;2;)
    (import "import-type-r" (type (;0;) (sub resource)))
    (import "import-type-r0" (type (;1;) (eq 0)))
    (import "import-type-r01" (type (;2;) (eq 1)))
    (type (;3;) (own 2))
    (type (;4;) (func (result 3)))
    (import "import-func-f" (func (;0;) (type 4)))
    (export (;5;) "r" (type 1))
    (type (;6;) (own 5))
    (type (;7;) (func (result 6)))
    (export (;1;) "f" (func 0) (func (type 7)))
  )
  (instance (;4;) (instantiate 2
      (with "import-func-f" (func 0))
      (with "import-type-r" (type 2))
      (with "import-type-r0" (type 3))
      (with "import-type-r01" (type 3))
    )
  )
  (@producers
    (processed-by "wit-component" "$CARGO_PKG_VERSION")
  )
  (export (;5;) "some-name" (instance 4))
)