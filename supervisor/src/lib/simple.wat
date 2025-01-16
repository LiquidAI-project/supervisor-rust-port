;; simple.wat
(module
  ;; A function (export "add") that adds two i32
  (func $add (export "add") (param $lhs i32) (param $rhs i32) (result i32)
    local.get $lhs
    local.get $rhs
    i32.add
  )
)
