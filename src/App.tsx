function App() {
  return (
    <div className="h-screen w-screen p-2">
      <div className="h-full w-full bg-neutral-900/95 backdrop-blur rounded-2xl shadow-2xl ring-1 ring-white/5 flex flex-col overflow-hidden text-neutral-100">
        <header
          data-tauri-drag-region
          className="h-10 px-4 flex items-center justify-between select-none cursor-grab active:cursor-grabbing border-b border-white/5"
        >
          <span data-tauri-drag-region className="text-sm font-medium">
            VibeIsland Linux
          </span>
          <span data-tauri-drag-region className="text-[10px] uppercase tracking-wide text-neutral-500">
            phase 0
          </span>
        </header>

        <main className="flex-1 p-4 overflow-y-auto">
          <p className="text-sm text-neutral-400 leading-relaxed">
            Scaffolding ready. The floating overlay is configured (decorations off,
            always-on-top, transparent). Next up: agent adapters and the session
            watcher.
          </p>
        </main>

        <footer className="h-8 px-4 flex items-center justify-between text-[10px] text-neutral-600 border-t border-white/5">
          <span>No active agents</span>
          <span>v0.1.0</span>
        </footer>
      </div>
    </div>
  );
}

export default App;
