// Benchmarks of go-hyperliquid (github.com/sonirico/go-hyperliquid).
//
// The same subcommands and JSON output as the Rust binaries in bench/:
//
//	sign-check                  the signature of the fixed order, for the cross-SDK check
//	sign                        hash and sign the fixed order; build, sign and serialize it
//	parse --corpus <path>       decode the recorded websocket frames as the SDK does
//	serve --px <p> --sz <s>     answer `info` and `exchange` lines on stdin, one JSON line each
//	live --secs <n>             BTC l2Book (fast), bbo and trades from mainnet for n seconds
//	build-info                  whether cgo was on, and which secp256k1 go-ethereum signs with
//
// The SDK's public SignL1Action takes the nonce as an argument, so it signs the fixed order
// directly. The order action is built with the SDK's public wire types, as its private
// newCreateOrderAction builds it; floatToWire is copied because it is private.
package main

import (
	"bufio"
	"compress/gzip"
	"context"
	"crypto/ecdsa"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"runtime"
	"slices"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/ethereum/go-ethereum/crypto"
	"github.com/mailru/easyjson/jlexer"
	hl "github.com/sonirico/go-hyperliquid"
)

const (
	sdk        = "go-hyperliquid"
	fixedKey   = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
	fixedNonce = int64(1_700_000_000_000)
	fixedAsset = 0
	fixedPx    = 50000.0
	fixedSz    = 0.001
)

// A quick run (BENCH_QUICK=1) checks that everything runs rather than measuring.
var quick = os.Getenv("BENCH_QUICK") == "1"

func pick(full, short int) int {
	if quick {
		return short
	}
	return full
}

var (
	signWarmup        = pick(2_000, 200)
	signSamples       = pick(20_000, 2_000)
	parseWarmupPasses = pick(3, 1)
	parsePasses       = pick(20, 5)
)

func flag(name string) string {
	args := os.Args[2:]
	for at, arg := range args {
		if arg == name && at+1 < len(args) {
			return args[at+1]
		}
	}
	return ""
}

func percentile(sorted []int64, fraction float64) int64 {
	if len(sorted) == 0 {
		return 0
	}
	rank := int(math.Round(float64(len(sorted)-1) * fraction))
	return sorted[min(rank, len(sorted)-1)]
}

func summaryNs(samples []int64) map[string]any {
	sorted := slices.Clone(samples)
	slices.Sort(sorted)
	var total float64
	for _, sample := range sorted {
		total += float64(sample)
	}
	mean := 0.0
	if len(sorted) > 0 {
		mean = total / float64(len(sorted))
	}
	rate := 0.0
	if mean > 0 {
		rate = 1e9 / mean
	}
	return map[string]any{
		"n":         len(sorted),
		"min_ns":    percentile(sorted, 0),
		"median_ns": percentile(sorted, 0.5),
		"p90_ns":    percentile(sorted, 0.9),
		"p99_ns":    percentile(sorted, 0.99),
		"max_ns":    percentile(sorted, 1),
		"mean_ns":   mean,
		"ops_per_s": rate,
	}
}

func timeEach(count int, run func() any) []int64 {
	samples := make([]int64, count)
	for at := range samples {
		start := time.Now()
		sink = run()
		samples[at] = time.Since(start).Nanoseconds()
	}
	return samples
}

func warm(count int, run func() any) {
	for range count {
		sink = run()
	}
}

// Keeps results alive so the compiler cannot drop the work.
var sink any

// floatToWire is a copy of the SDK's private floatToWire.
func floatToWire(x float64) (string, error) {
	rounded := fmt.Sprintf("%.8f", x)
	parsed, err := strconv.ParseFloat(rounded, 64)
	if err != nil {
		return "", err
	}
	if math.Abs(parsed-x) >= 1e-12 {
		return "", fmt.Errorf("float_to_wire causes rounding: %f", x)
	}
	if rounded == "-0.00000000" {
		rounded = "0.00000000"
	}
	result := strings.TrimRight(rounded, "0")
	result = strings.TrimRight(result, ".")
	return result, nil
}

// orderAction builds what newCreateOrderAction builds for one post-only BTC buy: the price and
// size through floatToWire, the asset looked up by coin, no client order id and no builder.
func orderAction(coinToAsset map[string]int, px, sz float64) (hl.OrderAction, error) {
	priceWire, err := floatToWire(px)
	if err != nil {
		return hl.OrderAction{}, err
	}
	sizeWire, err := floatToWire(sz)
	if err != nil {
		return hl.OrderAction{}, err
	}
	asset, ok := coinToAsset["BTC"]
	if !ok {
		return hl.OrderAction{}, fmt.Errorf("coin BTC not found")
	}
	order := hl.OrderWire{
		Asset:      asset,
		IsBuy:      true,
		LimitPx:    priceWire,
		Size:       sizeWire,
		ReduceOnly: false,
		OrderType:  hl.OrderWireType{Limit: &hl.OrderWireTypeLimit{Tif: hl.TifAlo}},
	}
	return hl.OrderAction{Type: "order", Orders: []hl.OrderWire{order}, Grouping: "na"}, nil
}

func fixed() (*ecdsa.PrivateKey, map[string]int, error) {
	key, err := crypto.HexToECDSA(strings.TrimPrefix(fixedKey, "0x"))
	return key, map[string]int{"BTC": fixedAsset}, err
}

// The SDK writes r and s without leading zeros; the check compares them padded to 32 bytes.
func hex65(signature hl.SignatureResult) string {
	pad := func(hex string) string {
		hex = strings.TrimPrefix(hex, "0x")
		return strings.Repeat("0", max(0, 64-len(hex))) + hex
	}
	return fmt.Sprintf("0x%s%s%02x", pad(signature.R), pad(signature.S), signature.V)
}

// buildInfo reports how this binary was built: go-ethereum picks its secp256k1 implementation
// with the build constraints copied in secp_cgo.go and secp_nocgo.go.
func buildInfo() (map[string]any, error) {
	return map[string]any{"cgo": cgoEnabled, "secp256k1": secp256k1Implementation, "go": runtime.Version()}, nil
}

func signCheck() (map[string]any, error) {
	key, coinToAsset, err := fixed()
	if err != nil {
		return nil, err
	}
	action, err := orderAction(coinToAsset, fixedPx, fixedSz)
	if err != nil {
		return nil, err
	}
	signature, err := hl.SignL1Action(key, action, "", fixedNonce, nil, true)
	if err != nil {
		return nil, err
	}
	// The action as the SDK posts it, read back so it prints as plain JSON.
	text, err := json.Marshal(action)
	if err != nil {
		return nil, err
	}
	var posted any
	if err := json.Unmarshal(text, &posted); err != nil {
		return nil, err
	}
	return map[string]any{
		"sdk":       sdk,
		"address":   strings.ToLower(crypto.PubkeyToAddress(key.PublicKey).Hex()),
		"signature": hex65(signature),
		"action":    posted,
	}, nil
}

func sign() (map[string]any, error) {
	key, coinToAsset, err := fixed()
	if err != nil {
		return nil, err
	}
	action, err := orderAction(coinToAsset, fixedPx, fixedSz)
	if err != nil {
		return nil, err
	}

	// Hash and sign an order action that is already built.
	hashAndSign := func() any {
		signature, _ := hl.SignL1Action(key, action, "", fixedNonce, nil, true)
		return signature
	}
	warm(signWarmup, hashAndSign)
	signed := timeEach(signSamples, hashAndSign)

	// From float price and size to the JSON body Exchange.postAction posts.
	buildSignBody := func() any {
		built, err := orderAction(coinToAsset, fixedPx, fixedSz)
		if err != nil {
			return nil
		}
		signature, err := hl.SignL1Action(key, built, "", fixedNonce, nil, true)
		if err != nil {
			return nil
		}
		payload := map[string]any{"action": built, "nonce": fixedNonce, "signature": signature}
		body, _ := json.Marshal(payload)
		return body
	}
	warm(signWarmup, buildSignBody)
	bodies := timeEach(signSamples, buildSignBody)

	return map[string]any{"sdk": sdk, "sign": summaryNs(signed), "build_sign_body": summaryNs(bodies)}, nil
}

type frame struct {
	channel string
	bytes   []byte
}

func loadCorpus(path string) ([]frame, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	unzipped, err := gzip.NewReader(file)
	if err != nil {
		return nil, err
	}
	var frames []frame
	lines := bufio.NewScanner(unzipped)
	lines.Buffer(make([]byte, 1<<20), 1<<24)
	for lines.Scan() {
		line := slices.Clone(lines.Bytes())
		if len(line) == 0 {
			continue
		}
		var head struct {
			Channel string `json:"channel"`
		}
		_ = json.Unmarshal(line, &head)
		frames = append(frames, frame{channel: head.Channel, bytes: line})
	}
	return frames, lines.Err()
}

// wsMessage is the SDK's private envelope type. Its UnmarshalJSON is a copy of the one easyjson
// generated for the SDK (ws_types_easyjson.go), so json.Unmarshal reads it the same way.
type wsMessage struct {
	Channel string          `json:"channel"`
	Data    json.RawMessage `json:"data"`
}

func (out *wsMessage) UnmarshalJSON(data []byte) error {
	in := jlexer.Lexer{Data: data}
	isTopLevel := in.IsStart()
	if in.IsNull() {
		if isTopLevel {
			in.Consumed()
		}
		in.Skip()
		return in.Error()
	}
	in.Delim('{')
	for !in.IsDelim('}') {
		key := in.UnsafeFieldName(false)
		in.WantColon()
		switch key {
		case "channel":
			if in.IsNull() {
				in.Skip()
			} else {
				out.Channel = string(in.String())
			}
		case "data":
			if in.IsNull() {
				in.Skip()
			} else {
				if data := in.Raw(); in.Ok() {
					in.AddError((out.Data).UnmarshalJSON(data))
				}
			}
		default:
			in.SkipRecursive()
		}
		in.WantComma()
	}
	in.Delim('}')
	if isTopLevel {
		in.Consumed()
	}
	return in.Error()
}

// decode is what the websocket client's read loop and its per-channel dispatcher do with each
// text frame: read the envelope, then read its data into the channel's type.
func decode(bytes []byte) bool {
	var message wsMessage
	if err := json.Unmarshal(bytes, &message); err != nil {
		return false
	}
	switch message.Channel {
	case hl.ChannelL2Book:
		var book hl.L2Book
		err := json.Unmarshal(message.Data, &book)
		sink = book
		return err == nil
	case hl.ChannelTrades:
		var trades hl.Trades
		err := json.Unmarshal(message.Data, &trades)
		sink = trades
		return err == nil
	case hl.ChannelBbo:
		var bbo hl.Bbo
		err := json.Unmarshal(message.Data, &bbo)
		sink = bbo
		return err == nil
	}
	return false
}

func throughput(frames []frame) map[string]any {
	bytes := 0
	failed := 0
	for _, f := range frames {
		bytes += len(f.bytes)
		if !decode(f.bytes) {
			failed++
		}
	}
	for range parseWarmupPasses {
		for _, f := range frames {
			decode(f.bytes)
		}
	}
	passes := make([]int64, 0, parsePasses)
	for range parsePasses {
		start := time.Now()
		for _, f := range frames {
			decode(f.bytes)
		}
		passes = append(passes, time.Since(start).Nanoseconds())
	}
	var total int64
	for _, pass := range passes {
		total += pass
	}
	seconds := float64(total) / 1e9
	slices.Sort(passes)
	frameNs := make([]int64, len(frames))
	for at, f := range frames {
		start := time.Now()
		decode(f.bytes)
		frameNs[at] = time.Since(start).Nanoseconds()
	}
	slices.Sort(frameNs)
	// One counted pass. Go's runtime counts heap allocations; values the compiler keeps on the
	// stack are not counted, as in the Rust rows.
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	for _, f := range frames {
		decode(f.bytes)
	}
	runtime.ReadMemStats(&after)
	count := float64(len(frames) * parsePasses)
	return map[string]any{
		"frames":                len(frames),
		"bytes":                 bytes,
		"failed":                failed,
		"passes":                parsePasses,
		"frames_per_s":          count / seconds,
		"mb_per_s":              float64(bytes*parsePasses) / seconds / 1e6,
		"median_pass_ns":        percentile(passes, 0.5),
		"ns_per_frame":          seconds * 1e9 / count,
		"frame_median_ns":       percentile(frameNs, 0.5),
		"frame_p99_ns":          percentile(frameNs, 0.99),
		"allocations_per_frame": float64(after.Mallocs-before.Mallocs) / float64(max(1, len(frames))),
	}
}

func parse() (map[string]any, error) {
	frames, err := loadCorpus(flag("--corpus"))
	if err != nil {
		return nil, err
	}
	only := func(channel string) []frame {
		var kept []frame
		for _, f := range frames {
			if f.channel == channel {
				kept = append(kept, f)
			}
		}
		return kept
	}
	return map[string]any{
		"sdk": sdk,
		"parse": map[string]any{
			"all":    throughput(frames),
			"l2Book": throughput(only("l2Book")),
			"trades": throughput(only("trades")),
		},
	}, nil
}

func serve() error {
	px, err := strconv.ParseFloat(flag("--px"), 64)
	if err != nil {
		return err
	}
	sz, err := strconv.ParseFloat(flag("--sz"), 64)
	if err != nil {
		return err
	}
	// A fresh random key that has never held anything: the venue has no account for it.
	key, err := crypto.GenerateKey()
	if err != nil {
		return err
	}
	address := strings.ToLower(crypto.PubkeyToAddress(key.PublicKey).Hex())
	ctx := context.Background()
	info := hl.NewInfo(ctx, hl.MainnetAPIURL, true, nil, nil, nil)
	exchange := hl.NewExchange(ctx, key, hl.MainnetAPIURL, nil, "", "", nil, nil)
	lines := bufio.NewScanner(os.Stdin)
	out := bufio.NewWriter(os.Stdout)
	for lines.Scan() {
		command := strings.TrimSpace(lines.Text())
		if command == "quit" || command == "" {
			break
		}
		var answer map[string]any
		start := time.Now()
		switch command {
		case "info":
			book, err := info.L2Snapshot(ctx, "BTC")
			ns := time.Since(start).Nanoseconds()
			if err != nil {
				answer = map[string]any{"ok": false, "ns": ns, "answer": err.Error()}
			} else {
				side := func(at int) int {
					if at < len(book.Levels) {
						return len(book.Levels[at])
					}
					return 0
				}
				answer = map[string]any{"ok": true, "ns": ns, "answer": fmt.Sprintf("%d bids %d asks", side(0), side(1))}
			}
		case "exchange":
			order := hl.CreateOrderRequest{
				Coin:      "BTC",
				IsBuy:     true,
				Price:     px,
				Size:      sz,
				OrderType: hl.OrderType{Limit: &hl.LimitOrderType{Tif: hl.TifAlo}},
			}
			status, err := exchange.Order(ctx, order, nil)
			ns := time.Since(start).Nanoseconds()
			if err != nil {
				// A venue refusal is the expected answer here.
				text := err.Error()
				refused := strings.Contains(text, "does not exist") || strings.Contains(text, "User or API Wallet")
				answer = map[string]any{"ok": refused, "ns": ns, "answer": text}
			} else {
				answer = map[string]any{"ok": true, "ns": ns, "answer": fmt.Sprintf("accepted: %+v", status)}
			}
		case "address":
			answer = map[string]any{"ok": true, "ns": 0, "answer": address}
		default:
			answer = map[string]any{"ok": false, "ns": 0, "answer": "unknown command " + command}
		}
		line, _ := json.Marshal(answer)
		out.Write(append(line, '\n'))
		out.Flush()
	}
	return lines.Err()
}

func live() (map[string]any, error) {
	secs, err := strconv.Atoi(flag("--secs"))
	if err != nil {
		return nil, err
	}
	var lock sync.Mutex
	tally := map[string]int{"book_frames": 0, "bbo_frames": 0, "trade_frames": 0, "trades": 0}
	var ages []int64
	aged := func(venueMs int64) {
		ages = append(ages, max(0, time.Now().UnixMilli()-venueMs))
	}
	deadline := time.Now().Add(time.Duration(secs) * time.Second)
	ctx := context.Background()
	ws := hl.NewWebsocketClient(hl.MainnetAPIURL)
	if err := ws.Connect(ctx); err != nil {
		return nil, err
	}
	if _, err := ws.L2Book(hl.L2BookSubscriptionParams{Coin: "BTC", Fast: true}, func(book hl.L2Book, err error) {
		if err != nil {
			return
		}
		lock.Lock()
		defer lock.Unlock()
		tally["book_frames"]++
		aged(book.Time)
	}); err != nil {
		return nil, err
	}
	if _, err := ws.Bbo(hl.BboSubscriptionParams{Coin: "BTC"}, func(bbo hl.Bbo, err error) {
		if err != nil {
			return
		}
		lock.Lock()
		defer lock.Unlock()
		tally["bbo_frames"]++
		aged(bbo.Time)
	}); err != nil {
		return nil, err
	}
	if _, err := ws.Trades(hl.TradesSubscriptionParams{Coin: "BTC"}, func(trades []hl.Trade, err error) {
		if err != nil || len(trades) == 0 {
			return
		}
		lock.Lock()
		defer lock.Unlock()
		tally["trade_frames"]++
		tally["trades"] += len(trades)
		newest := trades[0].Time
		for _, trade := range trades {
			newest = max(newest, trade.Time)
		}
		aged(newest)
	}); err != nil {
		return nil, err
	}
	time.Sleep(time.Until(deadline))
	lock.Lock()
	sorted := slices.Clone(ages)
	slices.Sort(sorted)
	report := map[string]any{
		"seconds":      secs,
		"book_frames":  tally["book_frames"],
		"bbo_frames":   tally["bbo_frames"],
		"trade_frames": tally["trade_frames"],
		"trades":       tally["trades"],
		"other_frames": nil,
		// The client logs a frame it cannot read only in debug mode, which also logs every
		// frame, so they are not counted.
		"unreadable":    nil,
		"connects":      1,
		"age_ms_median": percentile(sorted, 0.5),
		"age_ms_p99":    percentile(sorted, 0.99),
		"age_ms":        slices.Clone(ages),
	}
	lock.Unlock()
	_ = ws.Close()
	return map[string]any{"sdk": sdk, "live": report}, nil
}

func main() {
	command := ""
	if len(os.Args) > 1 {
		command = os.Args[1]
	}
	if command == "serve" {
		if err := serve(); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		return
	}
	commands := map[string]func() (map[string]any, error){
		"sign-check": signCheck,
		"sign":       sign,
		"parse":      parse,
		"live":       live,
		"build-info": buildInfo,
	}
	run, ok := commands[command]
	if !ok {
		fmt.Fprintln(os.Stderr, "usage: bench-go sign-check | sign | parse | serve | live | build-info")
		os.Exit(1)
	}
	report, err := run()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	line, err := json.Marshal(report)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Println(string(line))
}
