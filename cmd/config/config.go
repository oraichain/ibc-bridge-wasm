package config

import "os"

var (
	Bech32Prefix = os.Getenv("DENOM")
)
