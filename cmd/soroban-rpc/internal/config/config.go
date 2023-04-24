package config

import (
	"fmt"
	"os"
	"reflect"
	"runtime"
	"time"

	"github.com/sirupsen/logrus"

	"github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/network"
	support "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type LogFormat int

const (
	LogFormatText = iota
	LogFormatJSON
)

func (f LogFormat) String() string {
	switch f {
	case LogFormatText:
		return "text"
	case LogFormatJSON:
		return "json"
	default:
		panic(fmt.Sprintf("unknown log format: %d", f))
	}
}

type CaptiveCoreConfig = ledgerbackend.CaptiveCoreToml

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string `toml:"-" valid:"-"`

	// TODO: Enforce this when parsing this toml file
	Strict bool `toml:"STRICT" valid:"optional"`

	// TODO: Figure out what to do with these two flags. They conflict with the embedded captive-core config below
	StellarCoreURL   string `toml:"-" valid:"-"`
	CaptiveCoreUseDB bool   `toml:"-" valid:"-"`

	CaptiveCoreConfig `toml:"STELLAR_CORE" valid:"required"`

	// TODO: Is there a way to include these two in the CaptiveCoreConfig?
	CaptiveCoreStoragePath string `toml:"CAPTIVE_CORE_STORAGE_PATH" valid:"optional"`
	StellarCoreBinaryPath  string `toml:"STELLAR_CORE_BINARY_PATH" valid:"optional"`

	Endpoint                         string        `toml:"ENDPOINT" valid:"optional"`
	AdminEndpoint                    string        `toml:"ADMIN_ENDPOINT" valid:"optional"`
	CheckpointFrequency              uint32        `toml:"CHECKPOINT_FREQUENCY" valid:"optional"`
	CoreRequestTimeout               time.Duration `toml:"CORE_REQUEST_TIMEOUT" valid:"optional"`
	DefaultEventsLimit               uint          `toml:"DEFAULT_EVENTS_LIMIT" valid:"optional"`
	EventLedgerRetentionWindow       uint32        `toml:"EVENT_LEDGER_RETENTION_WINDOW" valid:"optional"`
	FriendbotURL                     string        `toml:"FRIENDBOT_URL" valid:"optional"`
	HistoryArchiveURLs               []string      `toml:"HISTORY_ARCHIVE_URLS" valid:"required"`
	IngestionTimeout                 time.Duration `toml:"INGESTION_TIMEOUT" valid:"optional"`
	LogFormat                        LogFormat     `toml:"LOG_FORMAT" valid:"optional"`
	LogLevel                         logrus.Level  `toml:"LOG_LEVEL" valid:"optional"`
	MaxEventsLimit                   uint          `toml:"MAX_EVENTS_LIMIT" valid:"optional"`
	MaxHealthyLedgerLatency          time.Duration `toml:"MAX_HEALTHY_LEDGER_LATENCY" valid:"optional"`
	NetworkPassphrase                string        `toml:"NETWORK_PASSPHRASE" valid:"required"`
	PreflightWorkerCount             uint          `toml:"PREFLIGHT_WORKER_COUNT" valid:"optional"`
	PreflightWorkerQueueSize         uint          `toml:"PREFLIGHT_WORKER_QUEUE_SIZE" valid:"optional"`
	SQLiteDBPath                     string        `toml:"SQLITE_DB_PATH" valid:"optional"`
	TransactionLedgerRetentionWindow uint32        `toml:"TRANSACTION_LEDGER_RETENTION_WINDOW" valid:"optional"`
}

func (cfg *Config) SetDefaults() {
	cfg.CaptiveCoreConfig.HTTPPort = 11626
	cfg.CaptiveCoreConfig.NetworkPassphrase = cfg.NetworkPassphrase
	cfg.CheckpointFrequency = 64
	cfg.CoreRequestTimeout = 2 * time.Second
	cfg.DefaultEventsLimit = 100
	cfg.Endpoint = "localhost:8000"
	cfg.EventLedgerRetentionWindow = uint32(ledgerbucketwindow.DefaultEventLedgerRetentionWindow)
	cfg.IngestionTimeout = 30 * time.Minute
	cfg.LogFormat = LogFormatText
	cfg.LogLevel = logrus.InfoLevel
	cfg.MaxEventsLimit = 10000
	cfg.MaxHealthyLedgerLatency = 30 * time.Second
	cfg.NetworkPassphrase = network.FutureNetworkPassphrase
	cfg.PreflightWorkerCount = uint(runtime.NumCPU())
	cfg.PreflightWorkerQueueSize = uint(runtime.NumCPU())
	cfg.SQLiteDBPath = "soroban_rpc.sqlite"
	cfg.TransactionLedgerRetentionWindow = 1440

	cwd, err := os.Getwd()
	if err != nil {
		panic(fmt.Errorf("unable to determine the current directory: %s", err))
	}
	cfg.CaptiveCoreStoragePath = cwd
}

func Read(path string) (*Config, error) {
	cfg := &Config{}
	// TODO: Enforce strict parsing here
	err := support.Read(path, cfg)
	if err != nil {
		switch cause := errors.Cause(err).(type) {
		case *support.InvalidConfigError:
			return nil, errors.Wrap(cause, "config file")
		default:
			return nil, err
		}
	}
	return cfg, nil
}

func (cfg *Config) Validate() error {
	if cfg.DefaultEventsLimit > cfg.MaxEventsLimit {
		return fmt.Errorf(
			"default-events-limit (%v) cannot exceed max-events-limit (%v)\n",
			cfg.DefaultEventsLimit,
			cfg.MaxEventsLimit,
		)
	}

	if len(cfg.HistoryArchiveURLs) == 0 {
		return cannotBeBlank(
			"history-archive-urls",
			"HISTORY_ARCHIVE_URLS",
		)
	}

	if cfg.NetworkPassphrase == "" {
		return cannotBeBlank(
			"network-passphrase",
			"NETWORK_PASSPHRASE",
		)
	}

	// if cfg.CaptiveCoreConfigPath == "" {
	// 	return cannotBeBlank(
	// 		"captive-core-config-path",
	// 		"CAPTIVE_CORE_CONFIG_PATH",
	// 	)
	// }
	if cfg.Strict && cfg.CaptiveCoreConfig.BucketDirPath != "" {
		return errors.New("could not unmarshal captive core toml: setting BUCKET_DIR_PATH is disallowed for Captive Core, use CAPTIVE_CORE_STORAGE_PATH instead")
	}
	// Validate home domains etc as in CaptiveCoreToml.validate

	if cfg.StellarCoreBinaryPath == "" {
		return cannotBeBlank(
			"stellar-core-binary-path",
			"STELLAR_CORE_BINARY_PATH",
		)
	}

	return nil
}

func cannotBeBlank(name, envVar string) error {
	return fmt.Errorf("Invalid config: %s is blank. Please specify --%s on the command line or set the %s environment variable.", name, name, envVar)
}

// Merge a and b, preferring values from b. Neither config is modified, instead
// a new struct is returned.
// TODO: Unit-test this
func mergeStructs(a, b reflect.Value) reflect.Value {
	if a.Type() != b.Type() {
		panic("Cannot merge structs of different types")
	}
	structType := a.Type()
	merged := reflect.New(structType).Elem()
	for i := 0; i < structType.NumField(); i++ {
		if !merged.Field(i).CanSet() {
			// Can't set unexported fields
			// TODO: Figure out how to fix this, cause this means it can't set the captiveCoreTomlValues
			continue
		}
		val := b.Field(i)
		if val.IsZero() {
			val = a.Field(i)
		}
		if val.Kind() == reflect.Struct {
			// Recurse into structs
			val = mergeStructs(a.Field(i), b.Field(i))
		}
		merged.Field(i).Set(val)

	}
	return merged
}

func (cfg Config) Merge(cfg2 Config) Config {
	return mergeStructs(
		reflect.ValueOf(cfg),
		reflect.ValueOf(cfg2),
	).Interface().(Config)
}
