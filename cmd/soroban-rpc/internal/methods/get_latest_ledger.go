package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

type GetLatestLedgerRequest struct{}

type GetLatestLedgerResponse struct {
	// Hash of the latest ledger as a hex-encoded string
	Hash string `json:"id"`
	// Stellar Core protocol version associated with the ledger.
	ProtocolVersion int `json:"protocolVersion,string"`
	// Sequence number of the latest ledger.
	Sequence uint32 `json:"sequence"`
}

// NewGetLatestLedgerHandler returns a JSON RPC handler to retrieve the latest ledger entry from Stellar core.
func NewGetLatestLedgerHandler(logger *log.Entry, ledgerEntryReader db.LedgerEntryReader, ledgerReader db.LedgerReader, coreClient *stellarcore.Client) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetLatestLedgerRequest) (GetLatestLedgerResponse, error) {
		tx, err := ledgerEntryReader.NewTx(ctx)
		if err != nil {
			return GetLatestLedgerResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not create read transaction",
			}
		}
		defer func() {
			_ = tx.Done()
		}()

		latestSequence, err := tx.GetLatestLedgerSequence()
		if err != nil {
			return GetLatestLedgerResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not get latest ledger sequence",
			}
		}

		latestLedger, found, err := ledgerReader.GetLedger(ctx, latestSequence)
		if (err != nil) || (!found) {
			return GetLatestLedgerResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not get latest ledger",
			}
		}

		info, err := coreClient.Info(ctx)
		if err != nil {
			return GetLatestLedgerResponse{}, (&jrpc2.Error{
				Code:    code.InternalError,
				Message: err.Error(),
			})
		}
		response := GetLatestLedgerResponse{
			Hash:            latestLedger.LedgerHash().HexString(),
			ProtocolVersion: info.Info.ProtocolVersion,
			Sequence:        latestSequence,
		}
		return response, nil
	})
}
