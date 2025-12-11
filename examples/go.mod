module zerobus-examples

go 1.25.3

require (
	google.golang.org/protobuf v1.35.2
	github.com/databricks/zerobus-go-sdk v0.0.0
)

// Use local zerobus module
replace github.com/databricks/zerobus-go-sdk => ../sdk
