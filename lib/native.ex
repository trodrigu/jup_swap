defmodule JupSwap.Native do

  mix_config = Mix.Project.config()
  version = mix_config[:version]
  github_url = mix_config[:package][:links]["GitHub"]
  mode = if Mix.env() in [:dev, :test], do: :debug, else: :release

  variants_for_linux = [
    legacy_cpu: fn ->
      # These are the same from the release workflow.
      # See the meaning in: https://unix.stackexchange.com/a/43540
      _needed_caps = ~w[fxsr sse sse2 ssse3 sse4_1 sse4_2 popcnt avx fma]
      # TODO: Implement logic using _needed_caps
      :ok
    end
  ]

  # other_variants = [legacy_cpu: fn -> use_legacy end]

  use RustlerPrecompiled,
    otp_app: :jup_swap,
    version: version,
    base_url: "#{github_url}/releases/download/v#{version}"|>IO.inspect(),
    targets: ~w(
      x86_64-apple-darwin
      x86_64-unknown-linux-gnu
    ),
    variants: %{
      "x86_64-unknown-linux-gnu" => variants_for_linux
    },
    # We don't use any features of newer NIF versions, so 2.15 is enough.
    nif_versions: ["2.15"],
    mode: mode,
    force_build: System.get_env("JUP_SWAP_BUILD") in ["1", "true"]

  def quick_swap(_token_to, _token_from, _amount), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end

