package main

import "github.com/b-harvest/gravity-dex-backend/cmd/gdex/cmd"

func main() {
	_ = cmd.RootCmd().Execute()
}
