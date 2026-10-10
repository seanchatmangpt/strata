defmodule Stratus.Valve.Arena do
  @moduledoc """
  Behaviour for the temprun actuation arena boundary. The valve shells out
  to a module implementing `actuate/1`; tests inject real funs or modules
  (no mocks).
  """

  @callback actuate(frame :: binary()) ::
              {:ok, %{required(:status) => atom(), required(:commit_hash) => binary()}}
              | {:error, atom()}
end
