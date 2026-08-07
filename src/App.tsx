import "./App.css";

function ShuttleMark() {
  return (
    <svg viewBox="0 0 48 48" role="img" aria-label="Suo">
      <path d="M24 4 41 20 24 44 7 20 24 4Z" />
      <path d="m8.5 20 15.5 6 15.5-6M24 26v17" />
    </svg>
  );
}

function App() {
  return (
    <main className="shell">
      <section className="card" aria-labelledby="product-title">
        <div className="mark"><ShuttleMark /></div>
        <p className="eyebrow">Cross-platform launcher</p>
        <h1 id="product-title">Suo <span>/ 梭</span></h1>
        <p className="summary">
          在应用、文件与个人命令之间快速穿梭。
        </p>
        <div className="status">
          <i aria-hidden="true" />
          <span>可编译基线 · 0.1.0</span>
        </div>
        <div className="features" aria-label="首版能力">
          <span>应用与文件</span>
          <span>计算与翻译</span>
          <span>脚本命令</span>
        </div>
      </section>
    </main>
  );
}

export default App;
