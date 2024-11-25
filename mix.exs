defmodule JupSwap.MixProject do
  use Mix.Project

  @source_url "https://github.com/trodrigu/jup_swap"
  @version "0.1.7"
  @dev? String.ends_with?(@version, "-dev")
  @force_build? System.get_env("JUP_SWAP_BUILD") in ["1", "true"]

  def project do
    [
      app: :jup_swap,
      version: @version,
      elixir: "~> 1.17.2",
      elixirc_paths: elixirc_paths(Mix.env()),
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      package: package()
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      extra_applications: [:logger],
      mod: {JupSwap.Application, []}
    ]
  end

  defp elixirc_paths(:test), do: ~w(lib test/support)
  defp elixirc_paths(_), do: ~w(lib)

  # Run "mix help deps" to learn about dependencies.
  defp deps do
    [
      {:rustler, "~> 0.34.0", optional: not (@dev? or @force_build?)},
      {:rustler_precompiled, "~> 0.7"},
      {:ex_doc, ">= 0.0.0", only: :dev, runtime: false}
      # {:dep_from_hexpm, "~> 0.3.0"},
      # {:dep_from_git, git: "https://github.com/elixir-lang/my_dep.git", tag: "0.1.0"}
    ]
  end

  defp package do
    [
      description: "An Elixir wrapper for the Jupiter Swap SDK",
      files: [
        "lib",
        "native",
        "checksum-*.exs",
        "mix.exs",
        "README.md",
        "LICENSE"
      ],
      licenses: ["MIT"],
      links: %{
        "GitHub" => @source_url,
        "Changelog" => "#{@source_url}/blob/v#{@version}/CHANGELOG.md"
      },
      maintainers: ["Thomas Rodriguez"],
      exclude_patterns: ["_build", "deps", "priv", "test", "notebooks", "doc", "native/jup_swap/target"]
    ]
  end
end
