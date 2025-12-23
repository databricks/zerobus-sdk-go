module zerobus-examples

go 1.25.3

require (
	github.com/databricks/zerobus-go-sdk v0.0.0
)

// Use local zerobus module
replace github.com/databricks/zerobus-go-sdk => ../../sdk
