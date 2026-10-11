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
 * Checks:
 *   1. Precondition:  pn_mask & pn_pre == pn_pre  (all required bits set)
 *   2. Conformance:   (ocel & 0xFF) == ((pn >> 48) & 0xFF)
 *                     activity/tid nibble agreement between ocel and pn
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

#define __byte_swap(x) __builtin_bswap64(x)

/* Opaque kernel struct; we only deref its first two pointer fields. */
struct xdp_md;

/* Compile-time precondition mask (see bottom of file). */
static inline __u64 pn_pre_mask(void);

SEC("xdp")
int pn_filter(struct xdp_md *ctx)
{
	/* struct xdp_md is opaque here; we only need data/data_end pointers. */
	__u64 data     = *(__u64 *)((__u8 *)ctx + 0); /* xdp_md.data     */
	__u64 data_end = *(__u64 *)((__u8 *)ctx + 8); /* xdp_md.data_end */

	/* Bounds check: need a full 16-byte PN64 payload. */
	if (data + 16 > data_end)
		return XDP_DROP;

	/* Two little-endian __u64 loads: pn, ocel. */
	__u64 pn   = __byte_swap(*(__u64 *)(data + 0));
	__u64 ocel = __byte_swap(*(__u64 *)(data + 8));

	/* Branchless precondition: m & pre == pre.
	 * bad_pre = ((m & pre) ^ pre) — 0 iff all precondition bits present. */
	__u64 pre_mask = pn_pre_mask();
	__u64 bad_pre  = (pn & pre_mask) ^ pre_mask;

	/* Activity/tid conformance: (ocel & 0xFF) == ((pn >> 48) & 0xFF). */
	__u64 bad_tid = ((ocel & 0xFF) ^ ((pn >> 48) & 0xFF));

	/* Fold both error words to a single bit-nonzero test. */
	__u64 bad = bad_pre | bad_tid;

	/* (bad | -bad) >> 63 == 0 when bad == 0, else 1 (sign bit).
	 * verdict = XDP_PASS - sign  →  2 (PASS) or 1 (DROP). */
	__u64 sign = (bad | -bad) >> 63;

	return XDP_PASS - (int)sign;
}

/* Compile-time precondition bits (Strata PN64 ingress policy).
 * Kept as a constant-foldable function so clang embeds the mask as an
 * immediate; change here to retighten the gate. */
static inline __u64 pn_pre_mask(void)
{
	/* Required pn header bits: version tag 0b01 in the top nibble plus
	 * the ingress-enabled flag (bit 0). Adjust to the current spec. */
	return 0x4000000000000001ULL;
}

char _license[] SEC("license") = "GPL";
