package main

import (
	"context"
	"encoding/json"
	"log"
	"os"

	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"

	"github.com/b-harvest/gravity-dex-backend/schema"
)

func main() {
	f, err := os.Open("banners.json")
	if err != nil {
		log.Fatalf("failed to open banners.json: %v", err)
	}
	defer f.Close()

	var banners []schema.Banner
	if err := json.NewDecoder(f).Decode(&banners); err != nil {
		log.Fatalf("failed to decode banners: %v", err)
	}

	mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI("mongodb://mongo"))
	if err != nil {
		log.Fatalf("failed connect to mongodb: %v", err)
	}
	defer mc.Disconnect(context.Background())

	coll := mc.Database("gdex").Collection("banners")
	var docs []interface{}
	for _, banner := range banners {
		docs = append(docs, banner)
	}
	r, err := coll.InsertMany(context.Background(), docs)
	if err != nil {
		log.Fatalf("failed to insert banners: %v", err)
	}
	log.Printf("imported %d banners", len(r.InsertedIDs))
}
