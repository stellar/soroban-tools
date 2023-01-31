package daemon

import (
	"net/http"
	"time"

	"github.com/jmoiron/sqlx"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest/ledgerbackend"
	supporthttp "github.com/stellar/go/support/http"
	supportlog "github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ingest"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

const (
	transactionProxyTTL          = 5 * time.Minute
	maxLedgerEntryWriteBatchSize = 150
)

type Daemon struct {
	core         *ledgerbackend.CaptiveStellarCore
	ingestRunner *ingest.Runner
	db           *sqlx.DB
	handler      *internal.Handler
	logger       *supportlog.Entry
}

func (d *Daemon) ServeHTTP(writer http.ResponseWriter, request *http.Request) {
	d.handler.ServeHTTP(writer, request)
}

func (d *Daemon) GetDB() *sqlx.DB {
	return d.db
}

func (d *Daemon) Close() error {
	var err error
	if localErr := d.ingestRunner.Close(); localErr != nil {
		err = localErr
	}
	if localErr := d.core.Close(); localErr != nil {
		err = localErr
	}
	d.handler.Close()
	if localErr := d.db.Close(); localErr != nil {
		err = localErr
	}
	return err
}

func MustNew(cfg config.LocalConfig) *Daemon {
	logger := supportlog.New()
	logger.SetLevel(cfg.LogLevel)

	httpPortUint := uint(cfg.CaptiveCoreHTTPPort)
	var captiveCoreTomlParams ledgerbackend.CaptiveCoreTomlParams
	captiveCoreTomlParams.HTTPPort = &httpPortUint
	captiveCoreTomlParams.HistoryArchiveURLs = cfg.HistoryArchiveURLs
	captiveCoreTomlParams.NetworkPassphrase = cfg.NetworkPassphrase
	captiveCoreTomlParams.Strict = true
	captiveCoreTomlParams.UseDB = cfg.CaptiveCoreUseDB
	captiveCoreToml, err := ledgerbackend.NewCaptiveCoreTomlFromFile(cfg.CaptiveCoreConfigPath, captiveCoreTomlParams)
	if err != nil {
		logger.WithError(err).Fatal("Invalid captive core toml")
	}

	captiveConfig := ledgerbackend.CaptiveCoreConfig{
		BinaryPath:          cfg.StellarCoreBinaryPath,
		StoragePath:         cfg.CaptiveCoreStoragePath,
		NetworkPassphrase:   cfg.NetworkPassphrase,
		HistoryArchiveURLs:  cfg.HistoryArchiveURLs,
		CheckpointFrequency: cfg.CheckpointFrequency,
		Log:                 logger.WithField("subservice", "stellar-core"),
		Toml:                captiveCoreToml,
		UserAgent:           "captivecore",
		UseDB:               cfg.CaptiveCoreUseDB,
	}
	core, err := ledgerbackend.NewCaptive(captiveConfig)
	if err != nil {
		logger.Fatalf("could not create captive core: %v", err)
	}

	historyArchive, err := historyarchive.Connect(
		cfg.HistoryArchiveURLs[0],
		historyarchive.ConnectOptions{
			CheckpointFrequency: cfg.CheckpointFrequency,
		},
	)
	if err != nil {
		logger.Fatalf("could not connect to history archive: %v", err)
	}

	dbConn, err := db.OpenSQLiteDB(cfg.SQLiteDBPath)
	if err != nil {
		logger.Fatalf("could not open database: %v", err)
	}

	ingestRunner, err := ingest.NewRunner(ingest.Config{
		Logger:            logger,
		DB:                db.NewWriter(dbConn, maxLedgerEntryWriteBatchSize, uint32(cfg.LedgerRetentionWindow)),
		NetworkPassPhrase: cfg.NetworkPassphrase,
		Archive:           historyArchive,
		LedgerBackend:     core,
		Timeout:           cfg.LedgerEntryStorageTimeout,
	})
	if err != nil {
		logger.Fatalf("could not initialize ledger entry writer: %v", err)
	}

	hc := &horizonclient.Client{
		HorizonURL: cfg.HorizonURL,
		HTTP: &http.Client{
			Timeout: horizonclient.HorizonTimeout,
		},
		AppName: "Soroban RPC",
	}
	hc.SetHorizonTimeout(horizonclient.HorizonTimeout)

	transactionProxy := methods.NewTransactionProxy(
		hc,
		cfg.TxConcurrency,
		cfg.TxQueueSize,
		cfg.NetworkPassphrase,
		transactionProxyTTL,
	)

	handler, err := internal.NewJSONRPCHandler(internal.HandlerParams{
		AccountStore:      methods.AccountStore{Client: hc},
		EventStore:        methods.EventStore{Client: hc},
		FriendbotURL:      cfg.FriendbotURL,
		NetworkPassphrase: cfg.NetworkPassphrase,
		Logger:            logger,
		TransactionProxy:  transactionProxy,
		CoreClient:        &stellarcore.Client{URL: cfg.StellarCoreURL},
		LedgerEntryReader: db.NewLedgerEntryReader(dbConn),
	})
	if err != nil {
		logger.Fatalf("could not create handler: %v", err)
	}
	handler.Start()
	return &Daemon{
		logger:       logger,
		core:         core,
		ingestRunner: ingestRunner,
		handler:      &handler,
		db:           dbConn,
	}
}

func Run(cfg config.LocalConfig, endpoint string) (exitCode int) {
	d := MustNew(cfg)
	supporthttp.Run(supporthttp.Config{
		ListenAddr: endpoint,
		Handler:    d,
		OnStarting: func() {
			d.logger.Infof("Starting Soroban JSON RPC server on %v", endpoint)
		},
		OnStopping: func() {
			d.Close()
		},
	})
	return 0
}
