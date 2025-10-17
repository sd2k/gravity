package main

import (
	"github.com/arcjet/gravity/examples/go-guest/arcjet/basic/basic"
	"go.bytecodealliance.org/cm"
	_ "go.bytecodealliance.org/x/cabi" // Import for cabi_realloc export
)

func init() {
	basic.Exports.Hello = Hello
	basic.Exports.Primitive = Primitive
	basic.Exports.OptionalPrimitive = OptionalPrimitive
	basic.Exports.ResultPrimitive = ResultPrimitive
	basic.Exports.OptionalString = OptionalString
}

func main() {}

func Hello() cm.Result[string, string, string] {
	return cm.OK[cm.Result[string, string, string]]("Hello, world!")
}

func Primitive() bool {
	return true
}

func OptionalPrimitive(b cm.Option[bool]) cm.Option[bool] {
	return cm.Some(true)
}

func ResultPrimitive() cm.Result[string, bool, string] {
	return cm.OK[cm.Result[string, bool, string]](true)
}

func OptionalString(s cm.Option[string]) cm.Option[string] {
	return s
}
