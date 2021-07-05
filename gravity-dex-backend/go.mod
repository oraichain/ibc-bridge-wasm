module github.com/b-harvest/gravity-dex-backend

go 1.16

require (
	github.com/cosmos/cosmos-sdk v0.42.4
	github.com/gomodule/redigo v1.8.4
	github.com/json-iterator/go v1.1.10
	github.com/labstack/echo/v4 v4.2.2
	github.com/spf13/cobra v1.1.3
	github.com/stretchr/testify v1.7.0
	github.com/tendermint/liquidity v1.2.4
	github.com/tendermint/tendermint v0.34.9
	go.mongodb.org/mongo-driver v1.5.1
	go.uber.org/zap v1.16.0
	golang.org/x/sync v0.0.0-20201020160332-67f06af15bc9
	gopkg.in/yaml.v2 v2.4.0
)

replace github.com/gogo/protobuf => github.com/regen-network/protobuf v1.3.3-alpha.regen.1
