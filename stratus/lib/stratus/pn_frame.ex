defmodule Stratus.PnFrame do
  @moduledoc """
  BEAM-tier 16-byte PN frame (lane L86): wire framing for the PN64/OCEL64
  dual-word law mirrored in `Stratus.Valve.Pn64`.

  Wire layout (fixed, little-endian, 16 bytes total — must stay bit-identical
  to the Rust `strata_protocol` ABI):

      bytes [7:0]   pn   :: 64-little  (PN64 word, see Stratus.Valve.Pn64)
      bytes [15:8]  ocel :: 64-little  (OCEL64 word)

  Decode fails closed: any input that is not exactly 16 bytes returns
  `{:error, :bad_frame}`.
  """

  @frame_size 16

  @doc """
  Encode a dual-word frame: `<<pn::64-little, ocel::64-little>>`.

  Arguments are masked to 64 bits by the binary construction itself; values
  above `2^64 - 1` are a caller contract violation.
  """
  @spec encode(pn :: non_neg_integer(), ocel :: non_neg_integer()) ::
          <<_::128>>
  def encode(pn, ocel) when is_integer(pn) and is_integer(ocel) and pn >= 0 and ocel >= 0 do
    <<pn::64-little, ocel::64-little>>
  end

  @doc """
  Decode exactly 16 bytes into `{:ok, pn, ocel}`; any other size fails closed
  with `{:error, :bad_frame}`.
  """
  @spec decode(binary()) ::
          {:ok, pn :: non_neg_integer(), ocel :: non_neg_integer()}
          | {:error, :bad_frame}
  def decode(frame) when is_binary(frame) and byte_size(frame) == @frame_size do
    <<pn::64-little, ocel::64-little>> = frame
    {:ok, pn, ocel}
  end

  def decode(_other), do: {:error, :bad_frame}

  @doc "True iff the frame decodes."
  @spec valid?(term()) :: boolean()
  def valid?(frame), do: match?({:ok, _, _}, decode(frame))
end
