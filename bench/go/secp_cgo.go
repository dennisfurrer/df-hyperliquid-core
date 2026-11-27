// The same build constraint as go-ethereum's crypto/signature_cgo.go: with cgo on, go-ethereum
// signs and recovers through its crypto/secp256k1 package, a cgo binding of libsecp256k1 (C).

//go:build !nacl && !js && !wasip1 && cgo && !gofuzz && !tinygo

package main

const cgoEnabled = true

const secp256k1Implementation = "libsecp256k1 (C) through cgo, go-ethereum crypto/secp256k1"
