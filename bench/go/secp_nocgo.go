// The same build constraint as go-ethereum's crypto/signature_nocgo.go: without cgo, go-ethereum
// signs and recovers with github.com/decred/dcrd/dcrec/secp256k1/v4, written in Go.

//go:build nacl || js || wasip1 || !cgo || gofuzz || tinygo

package main

const cgoEnabled = false

const secp256k1Implementation = "decred dcrd secp256k1/v4 (pure Go), go-ethereum without cgo"
