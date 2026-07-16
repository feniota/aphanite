<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600" class="h-full w-full">
  <!-- Background -->
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#6366f1" />
      <stop offset="100%" stop-color="#8b5cf6" />
    </linearGradient>
    <linearGradient id="card" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#4f46e5" />
      <stop offset="100%" stop-color="#7c3aed" />
    </linearGradient>
  </defs>
  <rect width="800" height="600" fill="url(#bg)" rx="0" />

  <!-- 装饰圆形 -->
  <circle cx="150" cy="120" r="140" fill="rgba(255,255,255,0.06)" />
  <circle cx="650" cy="480" r="200" fill="rgba(255,255,255,0.04)" />
  <circle cx="400" cy="300" r="90" fill="rgba(255,255,255,0.05)" />

  <!-- 服务器图标 -->
  <g transform="translate(180, 140)">
    <!-- 服务器机箱 -->
    <rect
      x="0"
      y="0"
      width="120"
      height="180"
      rx="12"
      fill="rgba(255,255,255,0.1)"
      stroke="rgba(255,255,255,0.2)"
      stroke-width="2" />
    <!-- 顶部指示灯 -->
    <circle cx="30" cy="25" r="6" fill="#34d399" />
    <circle cx="52" cy="25" r="6" fill="#fbbf24" />
    <circle cx="74" cy="25" r="6" fill="#f87171" />
    <!-- 硬盘槽 -->
    <rect x="16" y="50" width="88" height="16" rx="3" fill="rgba(255,255,255,0.15)" />
    <rect x="16" y="74" width="88" height="16" rx="3" fill="rgba(255,255,255,0.15)" />
    <rect x="16" y="98" width="88" height="16" rx="3" fill="rgba(255,255,255,0.1)" />
    <rect x="16" y="122" width="88" height="16" rx="3" fill="rgba(255,255,255,0.1)" />
    <!-- 底部通风口 -->
    <rect x="16" y="150" width="88" height="4" rx="2" fill="rgba(255,255,255,0.08)" />
    <rect x="16" y="158" width="88" height="4" rx="2" fill="rgba(255,255,255,0.08)" />
  </g>

  <!-- 数据库图标 -->
  <g transform="translate(430, 200)">
    <ellipse
      cx="60"
      cy="18"
      rx="60"
      ry="18"
      fill="rgba(255,255,255,0.15)"
      stroke="rgba(255,255,255,0.25)"
      stroke-width="2" />
    <path
      d="M0,18 L0,72 C0,91.882 26.863,108 60,108 C93.137,108 120,91.882 120,72 L120,18"
      fill="none"
      stroke="rgba(255,255,255,0.25)"
      stroke-width="2" />
    <ellipse
      cx="60"
      cy="72"
      rx="60"
      ry="18"
      fill="rgba(255,255,255,0.1)"
      stroke="rgba(255,255,255,0.2)"
      stroke-width="2" />
  </g>

  <!-- 盾牌/认证图标 -->
  <g transform="translate(320, 320)">
    <path
      d="M50,0 L100,20 L100,55 C100,78 75,95 50,100 C25,95 0,78 0,55 L0,20 Z"
      fill="rgba(255,255,255,0.12)"
      stroke="rgba(255,255,255,0.25)"
      stroke-width="2" />
    <!-- 对勾 -->
    <polyline
      points="25,50 42,67 78,32"
      fill="none"
      stroke="rgba(255,255,255,0.5)"
      stroke-width="5"
      stroke-linecap="round"
      stroke-linejoin="round" />
  </g>

  <!-- 文字 -->
  <text
    x="400"
    y="510"
    text-anchor="middle"
    fill="rgba(255,255,255,0.25)"
    font-family="system-ui, sans-serif"
    font-size="28"
    font-weight="700">
    aphanite
  </text>
  <text
    x="400"
    y="540"
    text-anchor="middle"
    fill="rgba(255,255,255,0.15)"
    font-family="system-ui, sans-serif"
    font-size="14">
    Minecraft Yggdrasil Auth Server
  </text>

  <!-- 连接线装饰 -->
  <line
    x1="260"
    y1="230"
    x2="370"
    y2="260"
    stroke="rgba(255,255,255,0.1)"
    stroke-width="1"
    stroke-dasharray="4 4" />
  <line
    x1="370"
    y1="260"
    x2="370"
    y2="320"
    stroke="rgba(255,255,255,0.1)"
    stroke-width="1"
    stroke-dasharray="4 4" />
  <line
    x1="490"
    y1="310"
    x2="420"
    y2="350"
    stroke="rgba(255,255,255,0.1)"
    stroke-width="1"
    stroke-dasharray="4 4" />
</svg>
