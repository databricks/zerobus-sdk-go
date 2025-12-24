module zerobus-examples

go 1.25.3

require (
	google.golang.org/protobuf v1.35.2
	github.com/databricks/zerobus-sdk-go v0.1.0
)

// Use local zerobus module
replace github.com/databricks/zerobus-sdk-go => ../..
