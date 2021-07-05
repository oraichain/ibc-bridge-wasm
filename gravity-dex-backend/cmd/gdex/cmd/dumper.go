package cmd

import (
	"context"
	"encoding/gob"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/spf13/cobra"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/pricetable"
	"github.com/b-harvest/gravity-dex-backend/service/score"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

func DumperCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dumper",
		Short: "state dumper",
	}
	cmd.AddCommand(DumperDumpCmd())
	cmd.AddCommand(DumperLoadCmd())
	return cmd
}

func DumperDumpCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dump",
		Short: "dump state",
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true

			cfg, err := config.Load("config.yml")
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}
			if err := cfg.Dumper.Validate(); err != nil {
				return fmt.Errorf("validate config: %w", err)
			}

			mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI(cfg.Dumper.MongoDB.URI))
			if err != nil {
				return fmt.Errorf("connect to mongodb: %w", err)
			}
			defer mc.Disconnect(context.Background())

			ss := store.NewService(cfg.Dumper.Store, mc)
			ps, err := price.NewService(cfg.Dumper.Price)
			if err != nil {
				return fmt.Errorf("new price service: %w", err)
			}
			pts := pricetable.NewService(cfg.Dumper.PriceTable, ps)
			scs := score.NewService(cfg.Dumper.Score, ss)

			d, err := NewDumper(cfg.Dumper, ss, ps, pts, scs)
			if err != nil {
				return fmt.Errorf("new dumper: %w", err)
			}

			started := time.Now()
			path, height, err := d.Dump(context.Background())
			if err != nil {
				return err
			}

			log.Printf("state dumped in %v (height = %d, path = %s)", time.Since(started), height, path)

			return nil
		},
	}
	return cmd
}

func DumperLoadCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "load [path]",
		Short: "load state",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true

			cfg, err := config.Load("config.yml")
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}
			if err := cfg.Dumper.Validate(); err != nil {
				return fmt.Errorf("validate config: %w", err)
			}

			d, err := NewDumper(cfg.Dumper, nil, nil, nil, nil)
			if err != nil {
				return fmt.Errorf("new dumper: %w", err)
			}

			started := time.Now()
			state, err := d.Load(args[0])
			if err != nil {
				return err
			}
			log.Printf("state loaded in %v (height = %d)", time.Since(started), state.BlockHeight)

			fmt.Println(state.Accounts)
			fmt.Println(state.Prices)

			return nil
		},
	}
	return cmd
}

type Dumper struct {
	cfg config.DumperConfig
	ss  *store.Service
	ps  price.Service
	pts *pricetable.Service
	scs *score.Service
}

func NewDumper(cfg config.DumperConfig, ss *store.Service, ps price.Service, pts *pricetable.Service, scs *score.Service) (*Dumper, error) {
	if err := os.MkdirAll(cfg.DumpDir, 0755); err != nil {
		return nil, fmt.Errorf("make dump dir: %w", err)
	}
	return &Dumper{cfg: cfg, ss: ss, ps: ps, pts: pts, scs: scs}, nil
}

func (d *Dumper) DumpStateFilename(blockHeight int64) string {
	return filepath.Join(d.cfg.DumpDir, fmt.Sprintf("%08d.dump", blockHeight))
}

func (d *Dumper) Dump(ctx context.Context) (string, int64, error) {
	blockHeight, err := d.ss.LatestBlockHeight(ctx)
	if err != nil {
		return "", 0, fmt.Errorf("get latest block height: %w", err)
	}
	pools, err := d.ss.Pools(ctx, blockHeight)
	if err != nil {
		return "", 0, fmt.Errorf("get pools: %w", err)
	}
	t, err := d.pts.PriceTable(ctx, pools)
	if err != nil {
		return "", 0, fmt.Errorf("get price table: %w", err)
	}
	accs, err := d.scs.Scoreboard(ctx, blockHeight, t)
	if err != nil {
		return "", 0, fmt.Errorf("get scoreboard: %w", err)
	}
	path := d.DumpStateFilename(blockHeight)
	f, err := os.Create(path)
	if err != nil {
		return "", 0, fmt.Errorf("create file: %w", err)
	}
	defer f.Close()
	if err := gob.NewEncoder(f).Encode(&DumpState{
		BlockHeight: blockHeight,
		Accounts:    accs,
		Prices:      t,
	}); err != nil {
		return "", 0, fmt.Errorf("write: %w", err)
	}
	return f.Name(), blockHeight, nil
}

func (d *Dumper) Load(path string) (*DumpState, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open: %w", err)
	}
	defer f.Close()
	var state DumpState
	if err := gob.NewDecoder(f).Decode(&state); err != nil {
		return nil, fmt.Errorf("decode: %w", err)
	}
	return &state, nil
}

type DumpState struct {
	BlockHeight int64
	Accounts    []score.Account
	Prices      price.Table
}
