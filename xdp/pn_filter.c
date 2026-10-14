/*
 * pn_filter.c — Strata PN64 ingress plane (PRD NFR-06)
 *
 * XDP program implementing the branchless PN64 precondition check at line
 * rate. Self-contained: no linux/bpf.h or bpf/bpf_helpers.h includes, so it
 * compiles with a plain `clang -target bpf -O2 -c` and no libbpf headers.
 *
 * SLO: 148 Mpps minimum sustainable throughput. The entire program is
 * straight-line ALU + one packet load pair; no loops, no map lookups, no
 * tail calls. Budget at 148 Mpps on a 10 GHz-class core: ~67 insns/packet
 * headroom — this program uses roughly 20.
 *
 * 16-byte wire layout (immediately after the L2/L3/L4 headers; caller
 * pins `data` at the PN64 payload offset):
 *
 *   offset 0..7   __le64  pn    — process number (little-endian on the wire)
 *   offset 8..15  __le64  ocel  — OCEL activity word (little-endian)
 *
 * Checks (all in the true LE register domain — the wire is little-endian
 * and the BPF target loads LE words natively, so no byte swap anywhere):
 *   1. Precondition:  (m & pre) == pre  with m = pn[15:0],
 *                     pre = pn[31:16] — the SAME predicate as the BEAM
 *                     valve's `Stratus.Valve.Pn64.enabled?/1` and the Rust
 *                     `pnabi` enablement law, on the same bits of the same
 *                     word carried in the frame.
 *   2. Conformance:   (ocel & 0xFF) == ((pn >> 48) & 0xFF)
 *                     activity/tid byte agreement between ocel and pn
 *
 * Both checks are combined into a single "bad bits" scalar and folded to a
 * verdict with the (x | -x) >> 63 trick — zero conditional branches on the
 * data path (speculation-free, single basic block).
 */

#define SEC(NAME) __attribute__((section(NAME), used))

typedef unsigned char  __u8;
typedef unsigned int   __u32;
typedef unsigned long long __u64;
typedef long long      __s64;

#define XDP_PASS 2
#define XDP_DROP 1

/* Opaque kernel struct; we only deref its first two pointer fields. */
struct xdp_md;

SEC("xdp")
int pn_filter(struct xdp_md *ctx)
{
	/* struct xdp_md is opaque here; we only need data/data_end pointers. */
	__u64 data     = *(__u64 *)((__u8 *)ctx + 0); /* xdp_md.data     */
	__u64 data_end = *(__u64 *)((__u8 *)ctx + 8); /* xdp_md.data_end */

	/* Bounds check: need a full 16-byte PN64 payload. */
	if (data + 16 > data_end)
		return XDP_DROP;

	/* Two little-endian __u64 loads: pn, ocel. The BPF target is
	 * little-endian, so a plain load of the LE wire word yields the
	 * numeric value directly — no bswap (a bswap here would move every
	 * later bit test into the byte-swapped domain and drop conformant
	 * frames; see xdp/README.md wire-layout law). */
	__u64 pn   = *(__u64 *)(data + 0);
	__u64 ocel = *(__u64 *)(data + 8);

	/* Branchless precondition: (m & pre) == pre — the valve's enablement
	 * law, evaluated on the frame's own fields (m = pn[15:0],
	 * pre = pn[31:16]). bad_pre = ((m & pre) ^ pre) — 0 iff all
	 * precondition places hold tokens. */
	__u64 m       = pn & 0xFFFFULL;
	__u64 pre     = (pn >> 16) & 0xFFFFULL;
	__u64 bad_pre = (m & pre) ^ pre;

	/* Activity/tid conformance: (ocel & 0xFF) == ((pn >> 48) & 0xFF). */
	__u64 bad_tid = ((ocel & 0xFF) ^ ((pn >> 48) & 0xFF));

	/* Fold both error words to a single bit-nonzero test. */
	__u64 bad = bad_pre | bad_tid;

	/* (bad | -bad) >> 63 == 0 when bad == 0, else 1 (sign bit).
	 * verdict = XDP_PASS - sign  →  2 (PASS) or 1 (DROP). */
	__u64 sign = (bad | -bad) >> 63;

	return XDP_PASS - (int)sign;
}

char _license[] SEC("license") = "GPL";
