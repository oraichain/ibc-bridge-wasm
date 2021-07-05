package main

import (
	"context"
	"encoding/csv"
	"errors"
	"io"
	"log"
	"os"
	"time"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"

	"github.com/b-harvest/gravity-dex-backend/schema"
)

func main() {
	f, err := os.Open("accounts.csv")
	if err != nil {
		log.Fatalf("failed to open accounts.csv: %v", err)
	}
	defer f.Close()

	now := time.Now()
	rd := csv.NewReader(f)
	var writes []mongo.WriteModel
	for {
		row, err := rd.Read()
		if err != nil {
			if errors.Is(err, io.EOF) {
				break
			}
			log.Fatalf("failed to read row: %v", err)
		}
		if len(row) == 2 {
			addr, username := row[0], row[1]
			writes = append(writes, mongo.NewUpdateOneModel().SetFilter(bson.M{
				schema.AccountAddressKey: addr,
			}).SetUpdate(bson.M{
				"$set": bson.M{
					schema.AccountUsernameKey:  username,
					schema.AccountCreatedAtKey: now,
				},
				"$setOnInsert": bson.M{
					schema.AccountIsBlockedKey: false,
				},
			}).SetUpsert(true))
		}
	}

	mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI("mongodb://mongo"))
	if err != nil {
		log.Fatalf("failed connect to mongodb: %v", err)
	}
	defer mc.Disconnect(context.Background())

	coll := mc.Database("gdex").Collection("accounts")
	log.Printf("importing %d account metadata", len(writes))
	if _, err := coll.BulkWrite(context.Background(), writes); err != nil {
		log.Fatalf("failed to insert account metadata: %v", err)
	}
	log.Print("done")
}
