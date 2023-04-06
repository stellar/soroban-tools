package daemon

import (
	"context"
	"errors"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/jmoiron/sqlx"

	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest/ledgerbackend"
	supportlog "github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ingest"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/preflight"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

const (
	maxLedgerEntryWriteBatchSize = 150
	defaultReadTimeout           = 5 * time.Second
	defaultShutdownGracePeriod   = 10 * time.Second
)

type Daemon struct {
	core                *ledgerbackend.CaptiveStellarCore
	ingestService       *ingest.Service
	db                  *sqlx.DB
	handler             *internal.Handler
	logger              *supportlog.Entry
	preflightWorkerPool *preflight.PreflightWorkerPool
}

func (d *Daemon) ServeHTTP(writer http.ResponseWriter, request *http.Request) {
	d.handler.ServeHTTP(writer, request)
}

func (d *Daemon) GetDB() *sqlx.DB {
	return d.db
}

func (d *Daemon) Close() error {
	var err error
	if localErr := d.ingestService.Close(); localErr != nil {
		err = localErr
	}
	if localErr := d.core.Close(); localErr != nil {
		err = localErr
	}
	d.handler.Close()
	if localErr := d.db.Close(); localErr != nil {
		err = localErr
	}
	d.preflightWorkerPool.Close()
	return err
}

// newCaptiveCore create a new captive core backend instance and returns it.
func newCaptiveCore(cfg *config.LocalConfig, logger *supportlog.Entry) (*ledgerbackend.CaptiveStellarCore, error) {
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
	return ledgerbackend.NewCaptive(captiveConfig)

}

func MustNew(cfg config.LocalConfig) *Daemon {
	logger := supportlog.New()
	logger.SetLevel(cfg.LogLevel)

	core, err := newCaptiveCore(&cfg, logger)
	if err != nil {
		logger.Fatalf("could not create captive core: %v", err)
	}

	if len(cfg.HistoryArchiveURLs) == 0 {
		logger.Fatalf("no history archives url were provided")
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

	eventStore := events.NewMemoryStore(cfg.NetworkPassphrase, cfg.EventLedgerRetentionWindow)
	transactionStore := transactions.NewMemoryStore(cfg.NetworkPassphrase, cfg.TransactionLedgerRetentionWindow)

	maxRetentionWindow := cfg.EventLedgerRetentionWindow
	if cfg.TransactionLedgerRetentionWindow > maxRetentionWindow {
		maxRetentionWindow = cfg.TransactionLedgerRetentionWindow
	} else if cfg.EventLedgerRetentionWindow == 0 && cfg.TransactionLedgerRetentionWindow > ledgerbucketwindow.DefaultEventLedgerRetentionWindow {
		maxRetentionWindow = ledgerbucketwindow.DefaultEventLedgerRetentionWindow
	}

	// initialize the stores using what was on the DB
	// TODO: add a timeout?
	txmetas, err := db.NewLedgerReader(dbConn).GetAllLedgers(context.Background())
	if err != nil {
		logger.Fatalf("could obtain txmeta cache from the database: %v", err)
	}
	for _, txmeta := range txmetas {
		// NOTE: We could optimize this to avoid unnecessary ingestion calls
		//       (len(txmetas) can be larger than the store retention windows)
		//       but it's probably not worth the pain.
		if err := eventStore.IngestEvents(txmeta); err != nil {
			logger.Fatalf("could initialize event memory store: %v", err)
		}
		if err := transactionStore.IngestTransactions(txmeta); err != nil {
			logger.Fatalf("could initialize transaction memory store: %v", err)
		}
	}

	ingestService, err := ingest.NewService(ingest.Config{
		Logger:            logger,
		DB:                db.NewReadWriter(dbConn, maxLedgerEntryWriteBatchSize, maxRetentionWindow),
		EventStore:        eventStore,
		TransactionStore:  transactionStore,
		NetworkPassPhrase: cfg.NetworkPassphrase,
		Archive:           historyArchive,
		LedgerBackend:     core,
		Timeout:           cfg.LedgerEntryStorageTimeout,
	})
	if err != nil {
		logger.Fatalf("could not initialize ledger entry writer: %v", err)
	}

	ledgerEntryReader := db.NewLedgerEntryReader(dbConn)
	preflightWorkerPool := preflight.NewPreflightWorkerPool(
		cfg.PreflightWorkerCount, cfg.PreflightWorkerQueueSize, ledgerEntryReader, cfg.NetworkPassphrase, logger)

	handler := internal.NewJSONRPCHandler(&cfg, internal.HandlerParams{
		EventStore:       eventStore,
		TransactionStore: transactionStore,
		Logger:           logger,
		CoreClient: &stellarcore.Client{
			URL:  cfg.StellarCoreURL,
			HTTP: &http.Client{Timeout: cfg.CoreRequestTimeout},
		},
		LedgerEntryReader: db.NewLedgerEntryReader(dbConn),
		PreflightGetter:   preflightWorkerPool,
	})

	return &Daemon{
		logger:              logger,
		core:                core,
		ingestService:       ingestService,
		handler:             &handler,
		db:                  dbConn,
		preflightWorkerPool: preflightWorkerPool,
	}
}

func Run(cfg config.LocalConfig, endpoint string) {
	d := MustNew(cfg)

	server := &http.Server{
		Addr:        endpoint,
		Handler:     d,
		ReadTimeout: defaultReadTimeout,
	}

	d.logger.Infof("Starting Soroban JSON RPC server on %v", endpoint)

	go func() {
		if err := server.ListenAndServe(); !errors.Is(err, http.ErrServerClosed) {
			// Error starting or closing listener:
			d.logger.Fatalf("Soroban JSON RPC server encountered fatal error: %v", err)
			os.Exit(1)
		}
		d.Close()
	}()

	// Shutdown gracefully when we receive an interrupt signal.
	// First server.Shutdown closes all open listeners, then closes all idle connections.
	// Finally, it waits a grace period (10s here) for connections to return to idle and then shut down.
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM)
	<-signals

	// Default supporthttp Shutdown grace period.
	shutdownCtx, shutdownRelease := context.WithTimeout(context.Background(), defaultShutdownGracePeriod)
	defer shutdownRelease()

	if err := server.Shutdown(shutdownCtx); err != nil {
		// Error from closing listeners, or context timeout:
		d.logger.Errorf("Error during Soroban JSON RPC server Shutdown: %v", err)
	}
}
