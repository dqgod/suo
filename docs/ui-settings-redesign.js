/* Local design prototype. No native bridge, network calls, or persistent storage. */
(() => {
  'use strict';
  const $ = (s, root = document) => root.querySelector(s);
  const $$ = (s, root = document) => [...root.querySelectorAll(s)];
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const clone = (value) => JSON.parse(JSON.stringify(value));
  const paths = {
    sliders:'<path d="M4 7h9m4 0h3M4 17h3m4 0h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
    search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 4 4"/>',
    code:'<path d="m8 7-5 5 5 5m8-10 5 5-5 5m-3-13-2 20"/>',
    globe:'<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/>',
    language:'<path d="M3 5h12M9 3v2m3 0c0 7-5 11-9 12m2-9c1 4 4 7 8 9m1 3 4-11 4 11m-6-4h5"/>',
    palette:'<circle cx="12" cy="12" r="9"/><circle cx="8" cy="8" r="1"/><circle cx="14" cy="7" r="1"/><circle cx="17" cy="12" r="1"/><path d="M5 16h6l2 5"/>',
    plus:'<path d="M12 5v14M5 12h14"/>',
    clock:'<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>',
    qr:'<path d="M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h3v3h3v3h-6z"/>',
    folder:'<path d="M3 6h7l2 3h9v11H3z"/>',
    terminal:'<path d="m5 7 5 5-5 5m8 0h6M3 3h18v18H3z"/>',
    arrow:'<path d="m9 5 7 7-7 7"/>',
    check:'<path d="m5 12 4 4L19 6"/>',
    info:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7v.1"/>',
    file:'<path d="M5 3h9l5 5v13H5zM14 3v6h5M8 13h8m-8 4h6"/>',
    external:'<path d="M14 3h7v7m0-7L10 14M10 5H4v15h15v-6"/>',
    keyboard:'<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h1m4 0h1m4 0h1M6 13h1m4 0h1m4 0h1M7 16h10"/>'
  };
  const icon = name => `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.code}</svg>`;
  const modes = {
    native:{number:'01 / 03',title:'让设置，像系统一样自然。',desc:'清爽的分组列表，编辑时打开独立面板。内容少一点、留白多一点，适合偶尔调整设置。',foot:'A · 浅色原生 / 分组列表 / 独立编辑面板'},
    studio:{number:'B / 细化稿',title:'少一些干扰，多一些空间。',desc:'透明图标融入界面，皮肤库按需展开。把主要空间留给当前设置与实时预览。',foot:'B · 分栏工具 / 可切换图标 / 紧凑皮肤库'},
    refined:{number:'03 / 03',title:'熟悉的深色，更清楚的层次。',desc:'保留当前的侧栏与折叠操作，减少嵌套和装饰。改动更小，日常操作更容易适应。',foot:'C · 深色改良 / 折叠编辑 / 更清晰的内容层级'}
  };
  const pages = {
    general:{title:'通用',desc:'让 Suo 按你的习惯工作。',group:'偏好',icon:'sliders'},
    search:{title:'搜索与索引',desc:'决定在哪里查找，以及何时开始搜索。',group:'偏好',icon:'search'},
    scripts:{title:'脚本命令',desc:'把重复的事情，交给一个关键词。',group:'能力',icon:'code'},
    web:{title:'网页搜索',desc:'从一个关键词，直达常用网站。',group:'能力',icon:'globe'},
    translation:{title:'翻译服务',desc:'选好服务，然后专注于你想表达的内容。',group:'能力',icon:'language'},
    appearance:{title:'外观',desc:'搜索界面与设置界面，各有自己的样子。',group:'个性化',icon:'palette'}
  };
  let commands = [
    {id:'timestamp',type:'scripts',name:'时间戳转换',keyword:'ts',aliases:'timestamp',description:'时间戳与日期，快速互转',path:'scripts/timestamp.py',runtime:'Python',enabled:true,icon:'clock',run:'enter',delay:50,timeout:10000,hint:'输入时间戳或日期，可留空获取当前时间',output:'纯文本',encoding:'UTF-8',action:'复制结果'},
    {id:'qr',type:'scripts',name:'生成二维码',keyword:'qr',aliases:'',description:'将文本或链接转成二维码',path:'scripts/qr.py',runtime:'Python',enabled:true,icon:'qr',run:'enter',delay:50,timeout:10000,hint:'输入文本或链接',output:'纯文本',encoding:'UTF-8',action:'复制结果'},
    {id:'workspace',type:'scripts',name:'打开工作目录',keyword:'work',aliases:'project',description:'直达常用项目文件夹',path:'scripts/workspace.ps1',runtime:'PowerShell',enabled:true,icon:'folder',run:'enter',delay:50,timeout:10000,hint:'',output:'纯文本',encoding:'UTF-8',action:'复制结果'},
    {id:'clipboard',type:'scripts',name:'清理剪贴板格式',keyword:'plain',aliases:'',description:'去掉多余的换行和富文本样式',path:'scripts/plain.ps1',runtime:'PowerShell',enabled:false,icon:'terminal',run:'enter',delay:50,timeout:10000,hint:'',output:'纯文本',encoding:'UTF-8',action:'复制结果'},
    {id:'google',type:'web',name:'Google',keyword:'g',aliases:'google',description:'搜索整个互联网',path:'https://www.google.com/search?q={query}',enabled:true,icon:'globe',hint:'输入搜索内容'},
    {id:'github',type:'web',name:'GitHub',keyword:'gh',aliases:'',description:'查找代码与仓库',path:'https://github.com/search?q={query}',enabled:true,icon:'code',hint:'输入项目或代码'},
    {id:'docs',type:'web',name:'MDN 文档',keyword:'mdn',aliases:'',description:'直接打开开发文档',path:'https://developer.mozilla.org/',enabled:true,icon:'file',hint:''}
  ];
  const params = new URLSearchParams(location.search);
  const state = {direction:modes[params.get('direction')] ? params.get('direction') : 'studio',page:pages[params.get('page')] ? params.get('page') : 'scripts',selected:'timestamp',editor:null,original:null,filter:'all',search:'',scope:'launcher',theme:{launcher:{selected:'midnight',active:'midnight',custom:null},settings:{selected:'paper',active:'paper',custom:null}},themeDraft:null,themeOriginal:null,ordinary:{auto:true,startup:false,closeBlur:true,keepInput:false,emptyDelay:0,queryDelay:50,width:720,height:520,hotkey:'Alt + Space'},ordinarySaved:null,translation:{provider:'Microsoft Translator',keyword:'fy',target:'en'},translationSaved:null};
  state.ordinarySaved=clone(state.ordinary);state.translationSaved=clone(state.translation);
  state.ordinary.iconStyle='color';state.ordinarySaved.iconStyle='color';
  state.library={filter:'all',search:''};
  const seedThemes=[
    ['mist','雾白','paper','#496f67'],['moss','苔绿','forest','#365847'],['slate','石墨','midnight','#4e596e'],
    ['dawn','晨光','paper','#986843'],['iris','鸢尾','midnight','#684b83'],['ocean','深海','midnight','#3c5482'],
    ['linen','亚麻','paper','#847259'],['pine','松影','forest','#496147'],['rose','玫瑰灰','paper','#82566c'],['night','夜航','midnight','#415076']
  ];
  for(const scope of ['launcher','settings'])state.theme[scope].customThemes=seedThemes.map(([id,name,base,accent])=>({id,name,base,accent,radius:12,fontSize:14}));
  function customTheme(id=state.theme[state.scope].selected){const s=state.theme[state.scope];return id==='custom'?s.custom:s.customThemes.find(t=>t.id===id)||null}
  function themeName(id){return themeNames[id]||customTheme(id)?.name||'未命名皮肤'}
  function libraryThemes(){const s=state.theme[state.scope];return [...['midnight','paper','forest'].map(id=>({id,name:themeNames[id],base:id,kind:'builtin'})),...s.customThemes.map(t=>({...t,kind:'custom'})),...(s.custom?[{...s.custom,id:'custom',name:'我的皮肤',kind:'custom'}]:[])]}
  let markSequence=0;
  function brandMarkup(style=state.ordinary.iconStyle){
    if(style==='original')return '<img class="suo-brand-mark" src="../src-tauri/icons/icon.png" alt="">';
    const id=`suo-mark-gradient-${++markSequence}`;
    return `<svg class="suo-brand-mark ${style==='mono'?'monochrome':'color-mark'}" viewBox="80 44 352 424" aria-hidden="true"><defs><linearGradient id="${id}" x1="128" y1="80" x2="386" y2="438" gradientUnits="userSpaceOnUse"><stop class="mark-top" stop-color="#8670eb"/><stop offset=".55" class="mark-middle" stop-color="#7463ed"/><stop offset="1" class="mark-bottom" stop-color="#369dcc"/></linearGradient></defs><g fill="none" stroke="${style==='mono'?'currentColor':`url(#${id})`}" stroke-linecap="round" stroke-linejoin="round"><path d="M256 70 408 210 256 442 104 210 256 70Z" stroke-width="23"/><path d="m108 210 148 58 148-58M256 268v168" stroke-width="19"/></g><circle cx="256" cy="268" r="13" fill="${style==='mono'?'currentColor':'#c0b3ff'}" stroke="none"/></svg>`;
  }
  function updateBrand(){
    $$('[data-brand-slot]').forEach(slot=>slot.innerHTML=brandMarkup());
    $$('[data-brand-style]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.brandStyle===state.ordinary.iconStyle)));
    $('#brand-live-light')?.replaceChildren();$('#brand-live-dark')?.replaceChildren();
    if($('#brand-live-light'))$('#brand-live-light').innerHTML=brandMarkup()+'<span>Suo 设置</span>';
    if($('#brand-live-dark'))$('#brand-live-dark').innerHTML=brandMarkup()+'<span>Suo 设置</span>';
  }
  function brandSettings(){return `<h3 class="section-heading">品牌图标</h3><section class="settings-group brand-settings"><div class="setting-row"><div><strong>图标样式</strong><p>设置窗口的标题栏与侧栏使用同一种样式。</p></div><span class="subtle-badge">即时预览</span></div><div class="brand-options">${[['color','透明彩色','只保留图形，保留品牌色'],['mono','主题单色','只保留图形，颜色跟随界面'],['original','原版底板','保留深色底板与光晕']].map(([id,name,desc])=>`<button class="brand-option" data-brand-style="${id}" aria-pressed="${state.ordinary.iconStyle===id}"><span class="brand-option-sample">${brandMarkup(id)}</span><strong>${name}</strong><small>${desc}</small><span class="option-check" aria-hidden="true">${icon('check')}</span></button>`).join('')}</div><div class="brand-preview-row"><span>在不同背景上</span><div class="brand-live-light" id="brand-live-light">${brandMarkup()}<span>Suo 设置</span></div><div class="brand-live-dark" id="brand-live-dark">${brandMarkup()}<span>Suo 设置</span></div></div></section>`}
  let pendingAction=null,toastTimer,sequence=0;
  const editorDirty=()=>!!state.editor && JSON.stringify(state.editor)!==JSON.stringify(state.original);
  const themeDirty=()=>!!state.themeDraft && JSON.stringify(state.themeDraft)!==JSON.stringify(state.themeOriginal);
  const ordinaryDirty=()=>JSON.stringify(state.ordinary)!==JSON.stringify(state.ordinarySaved);
  const translationDirty=()=>JSON.stringify(state.translation)!==JSON.stringify(state.translationSaved);
  const anyDirty=()=>editorDirty()||themeDirty()||ordinaryDirty()||translationDirty();
  function toast(message){$('#toast').textContent=message;$('#toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').hidden=true,2800)}
  function notice(message){$('#notice-copy').textContent=message;$('#notice-dialog').showModal()}
  function syncUrl(){const u=new URL(location.href);u.searchParams.set('direction',state.direction);u.searchParams.set('page',state.page);history.replaceState(null,'',u)}
  function updateSaveBar(){
    const e=editorDirty(),t=themeDirty(),o=ordinaryDirty(),tr=translationDirty();
    $('#save-message').classList.toggle('pending',e||t||o||tr);
    $('#save-message').innerHTML=e?'● 当前命令有未保存的修改':t?'● 皮肤有未保存的修改':tr?'● 翻译配置有未保存的修改':o?'● 普通设置有未保存的修改':`${icon('check')} 所有更改已保存 <span class="demo-state">· 演示</span>`;
    $('#mode-label').textContent=state.ordinary.auto?'普通设置自动保存':'普通设置手动保存';
    $('#footer-actions .global-save')?.remove();
    if(o) $('#footer-actions').insertAdjacentHTML('afterbegin','<button class="button primary global-save" data-action="save-general">保存设置</button>');
    $$('.save-command').forEach(b=>b.disabled=!e);
  }
  function guard(action, closing=false){
    if(editorDirty()||themeDirty()||translationDirty()||(closing&&ordinaryDirty())){
      pendingAction=action;$('#guard-copy').textContent=editorDirty()?'当前命令有未保存的修改。离开前，你希望如何处理？':themeDirty()?'当前皮肤有未保存的修改。预览不会自动生效。':'当前设置有未保存的修改。离开前，你希望如何处理？';$('#guard-dialog').showModal();
    } else action();
  }
  function discardCurrent(){
    if(editorDirty()) state.editor=clone(state.original);
    if(themeDirty()) state.themeDraft=clone(state.themeOriginal);
    if(translationDirty()) state.translation=clone(state.translationSaved);
    if(ordinaryDirty()) state.ordinary=clone(state.ordinarySaved);
    updateSaveBar();
  }
  function saveCurrent(){
    if($('[data-theme-setting][aria-invalid="true"]'))return false;
    if(editorDirty()&&!saveCommand())return false;
    if(themeDirty())saveTheme();
    if(translationDirty())state.translationSaved=clone(state.translation);
    if(ordinaryDirty())state.ordinarySaved=clone(state.ordinary);
    updateSaveBar();return true;
  }
  function openEditor(id){
    const item=commands.find(c=>c.id===id);if(!item)return;
    state.selected=id;state.editor=clone(item);state.original=clone(item);
    renderCommands();
    if(state.direction==='native'){$('#dialog-editor').innerHTML=editorMarkup();$('#editor-dialog').showModal()}
    updateSaveBar();
  }
  function navigate(page){guard(()=>{if($('#editor-dialog').open)$('#editor-dialog').close();state.page=page;state.filter='all';state.search='';state.editor=null;state.original=null;render()})}
  function render(){
    $('#app').dataset.direction=state.direction;
    updateBrand();
    const mode=modes[state.direction];$('#direction-number').textContent=mode.number;$('#direction-title').textContent=mode.title;$('#direction-description').textContent=mode.desc;$('#direction-footnote').textContent=mode.foot;
    $$('.directions button').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.direction===state.direction)));
    let last='';$('#navigation').innerHTML=Object.entries(pages).map(([key,p])=>{const label=p.group!==last?`<div class="nav-group-label">${p.group}</div>`:'';last=p.group;return `${label}<button class="nav-item ${state.page===key?'active':''}" data-page="${key}" ${state.page===key?'aria-current="page"':''}>${icon(p.icon)}${p.title}${['scripts','web'].includes(key)?`<span class="count">${commands.filter(c=>c.type===key).length}</span>`:''}</button>`}).join('');
    const p=pages[state.page];$('#breadcrumb').textContent=`${p.group} / ${p.title}`;$('#page-title').textContent=p.title;$('#page-description').textContent=p.desc;
    $('#page-actions').innerHTML=['scripts','web'].includes(state.page)?`<button class="button primary" data-action="add">${icon('plus')} 添加${state.page==='scripts'?'命令':'搜索'}</button>`:'';
    if(['scripts','web'].includes(state.page))renderCommands();
    if(state.page==='general')renderGeneral();if(state.page==='search')renderSearch();if(state.page==='appearance')renderAppearance();if(state.page==='translation')renderTranslation();
    $('#page-content').scrollTop=0;updateSaveBar();syncUrl();
  }
  function commandList(){return commands.filter(c=>c.type===state.page).filter(c=>state.filter==='all'||(state.filter==='enabled'?c.enabled:!c.enabled)).filter(c=>[c.name,c.keyword,c.aliases,c.description].join(' ').toLowerCase().includes(state.search.toLowerCase()))}
  function itemMarkup(c){return `<span class="command-glyph">${icon(c.icon)}</span><div><strong>${esc(c.name)}</strong><small><code class="key">${esc(c.keyword)}</code><span>·</span>${c.runtime||'网页搜索'}</small></div><span class="status-dot ${c.enabled?'':'off'}" aria-label="${c.enabled?'已启用':'已停用'}"></span>`}
  function renderCommands(){
    const all=commands.filter(c=>c.type===state.page);const list=commandList();
    if(state.direction!=='native'&&(!state.editor||state.editor.type!==state.page)){
      const c=list.find(c=>c.id===state.selected)||list[0];state.editor=c?clone(c):null;state.original=c?clone(c):null;state.selected=c?.id||null;
    }
    const toolbar=`<div class="command-toolbar"><div class="filter-tabs" aria-label="命令状态筛选"><button data-filter="all" class="${state.filter==='all'?'active':''}">全部 <small>${all.length}</small></button><button data-filter="enabled" class="${state.filter==='enabled'?'active':''}">已启用 <small>${all.filter(c=>c.enabled).length}</small></button><button data-filter="disabled" class="${state.filter==='disabled'?'active':''}">已停用</button></div><label class="search-box">${icon('search')}<input id="command-search" type="search" value="${esc(state.search)}" aria-label="查找命令" placeholder="查找名称或关键词"></label></div>`;
    let content='';
    if(state.direction==='native')content=`<div class="native-list">${list.map(c=>`<article class="native-item"><span class="command-glyph">${icon(c.icon)}</span><div><h3>${esc(c.name)} <code class="key">${esc(c.keyword)}</code></h3><p>${esc(c.description)}</p></div><div class="row-meta"><span class="runtime-label">${c.runtime||'网页搜索'}</span><label class="enable-control"><input type="checkbox" class="toggle" data-enable="${c.id}" ${c.enabled?'checked':''} aria-label="启用${esc(c.name)}"></label><button class="button row-edit" data-command="${c.id}">编辑 ${icon('arrow')}</button></div></article>`).join('')||'<div class="empty-state">没有匹配的命令。试试其他名称或关键词。</div>'}</div><div class="native-guide">${icon('info')}<span>关键词与别名在所有命令中唯一。点击「编辑」调整细节，保存前不会影响正在使用的命令。</span></div>`;
    if(state.direction==='studio')content=`<div class="command-workbench"><div class="command-list"><div class="list-heading"><span>${state.page==='scripts'?'我的脚本':'我的搜索'}</span><span>${list.length} 项</span></div>${list.map(c=>`<button class="command-item ${c.id===state.selected?'selected':''}" data-command="${c.id}" ${c.id===state.selected?'aria-current="true"':''}>${itemMarkup(c)}</button>`).join('')||'<div class="empty-state">没有匹配项</div>'}</div><div id="inline-editor">${state.editor?editorMarkup():'<div class="empty-selection">添加一个命令，开始配置。</div>'}</div></div>`;
    if(state.direction==='refined')content=`<div class="refined-list">${list.map(c=>`<article class="accordion-card ${c.id===state.selected?'selected':''}"><button class="accordion-head" data-command="${c.id}" aria-expanded="${c.id===state.selected}"><span class="command-glyph">${icon(c.icon)}</span><div><strong>${esc(c.name)}</strong><small>${esc(c.description)}</small></div><code class="key">${esc(c.keyword)}</code><span class="badge">${c.enabled?'已启用':'已停用'}</span><span>${c.id===state.selected?'−':'＋'}</span></button>${c.id===state.selected?editorMarkup():''}</article>`).join('')||'<div class="empty-state">没有匹配的命令</div>'}</div>`;
    $('#page-content').innerHTML=toolbar+content;updateSaveBar();
  }
  function field(label,key,value,wide=false,extra=''){return `<label class="field ${wide?'wide':''}"><span>${label}</span><input data-field="${key}" value="${esc(value)}" ${extra}>${key==='keyword'||key==='aliases'?`<small class="field-error" data-error="${key}" role="alert"></small>`:''}</label>`}
  function editorMarkup(){
    const c=state.editor;if(!c)return'';const script=c.type==='scripts';
    return `<section class="command-editor" aria-label="命令编辑"><div class="editor-top"><div class="editor-identity"><span class="command-glyph">${icon(c.icon)}</span><div><h3>${esc(c.name||'新命令')}</h3><small>${script?'本地脚本':'网页搜索'} · ${c.runtime||'浏览器'}</small></div></div><label class="enable-control">启用<input class="toggle" data-field="enabled" type="checkbox" ${c.enabled?'checked':''}></label></div><div class="editor-body"><div class="eyebrow">基本信息</div><div class="form-grid">${field('名称','name',c.name)}${field('关键词','keyword',c.keyword,false,'aria-describedby="keyword-guidance"')}<label class="field wide"><span>描述 <span class="optional">· 可选</span></span><input data-field="description" value="${esc(c.description)}" maxlength="200"></label>${script?`<label class="field path-field"><span>脚本文件</span><div class="input-with-action"><input data-field="path" value="${esc(c.path)}"><button class="button" data-action="pick-file" aria-label="选择本地脚本">${icon('folder')}</button></div></label><label class="field"><span>运行环境</span><select data-field="runtime">${['Python','PowerShell','Bash','Executable'].map(r=>`<option ${r===c.runtime?'selected':''}>${r}</option>`).join('')}</select></label>`:`${field('网址','path',c.path,true)}<small class="field wide">{query} 插入完整搜索内容；没有占位符时，输入关键词即可打开链接。</small>`}</div><div class="eyebrow section-gap">${script?'运行方式':'输入与结果'}</div><div class="form-grid">${script?`<div class="field"><span>何时运行</span><div class="run-options"><button data-run="enter" class="${c.run==='enter'?'active':''}" aria-pressed="${c.run==='enter'}">按回车后</button><button data-run="immediate" class="${c.run==='immediate'?'active':''}" aria-pressed="${c.run==='immediate'}">输入停顿后</button></div></div><label class="field" id="delay-field"><span>${c.run==='immediate'?'输入停顿延迟':'运行超时'}</span><span class="unit-input"><input type="number" data-field="${c.run==='immediate'?'delay':'timeout'}" value="${c.run==='immediate'?c.delay:c.timeout}" min="${c.run==='immediate'?20:1}" max="60000"><span>ms</span></span><small class="field-error" data-error="timing" role="alert"></small></label>`:field('空输入提示 · 可选','hint',c.hint,true)}${field('别名 · 用逗号分隔','aliases',c.aliases,true)}</div><small id="keyword-guidance" class="field-note">关键词与别名不能和其他脚本、网页搜索或翻译命令重复。</small><details class="advanced" ${script?'open':''}><summary>更多设置 <span>图标、${script?'结果与安全':'输入提示'}</span></summary><div class="form-grid">${script?`${field('空输入提示 · 可选','hint',c.hint,true)}<label class="field"><span>输出协议</span><select data-field="output"><option ${c.output==='纯文本'?'selected':''}>纯文本</option><option ${c.output==='suo-json-v1'?'selected':''}>suo-json-v1</option></select></label><label class="field"><span>文本编码</span><select data-field="encoding"><option>UTF-8</option><option ${c.encoding==='GBK'?'selected':''}>GBK</option></select></label><label class="field wide"><span>结果动作</span><select data-field="action"><option>复制结果</option><option ${c.action==='执行 Shell 命令'?'selected':''}>执行 Shell 命令</option></select><small data-shell-warning ${c.action==='执行 Shell 命令'?'':'hidden'}>高风险：结果将作为本机命令执行，正式实现需再次确认。</small></label>`:''}<div class="field wide"><span>命令图标</span><div><button class="button" data-action="pick-icon">${icon('plus')} 选择本地图标</button></div><small>PNG / JPEG / WebP · 最大 256 KB、512 × 512 px</small></div></div></details><div class="invocation">${icon('terminal')}<span>调用示例</span><code id="invocation-code">${esc(c.keyword||'keyword')}${script?' 1727433600':c.path.includes('{query}')?' 搜索内容':''}</code></div></div><footer class="editor-footer"><small>修改仅在保存后生效</small><div><button class="button" data-action="discard-command">${state.direction==='native'?'取消':'还原'}</button><button class="button primary save-command" data-action="save-command" ${editorDirty()?'':'disabled'}>保存命令</button></div></footer></section>`;
  }
  function validateCommand(){
    const c=state.editor;if(!c)return false;const others=commands.filter(x=>x.id!==c.id).flatMap(x=>[x.keyword,...x.aliases.split(/[,，]/)]).concat(state.translation.keyword).map(x=>x.trim().toLowerCase()).filter(Boolean);
    const kw=c.keyword.trim();let error=!kw?'请填写关键词':/\s/.test(kw)?'关键词不能包含空格':others.includes(kw.toLowerCase())?`「${kw}」已被其他命令使用，请更换关键词。`:'';
    const aliases=c.aliases.split(/[,，]/).map(x=>x.trim()).filter(Boolean);const seen=new Set([kw.toLowerCase()]);let aliasError='';for(const alias of aliases){const n=alias.toLowerCase();if(/\s/.test(alias)||others.includes(n)||seen.has(n)){aliasError=`别名「${alias}」重复或包含空格。`;break}seen.add(n)}
    $$('[data-error="keyword"]').forEach(x=>x.textContent=error);$$('[data-field="keyword"]').forEach(x=>x.setAttribute('aria-invalid',String(!!error)));$$('[data-error="aliases"]').forEach(x=>x.textContent=aliasError);
    let timing='';if(c.type==='scripts'){const value=c.run==='immediate'?Number(c.delay):Number(c.timeout);const min=c.run==='immediate'?20:1;if(!Number.isInteger(value)||value<min||value>60000)timing=`请输入 ${min}–60000 之间的整数。`}
    $$('[data-error="timing"]').forEach(x=>x.textContent=timing);
    if(!c.name.trim()||!c.path.trim()){toast('名称和路径不能为空。');return false}
    if(c.type==='web'){try{const u=new URL(c.path.replace(/\{query\d*\}/g,'example'));if(!['http:','https:'].includes(u.protocol))throw Error()}catch{toast('请输入有效的 HTTP 或 HTTPS 网址。');return false}}
    if(error||aliasError||timing){(error?$('[data-field="keyword"]'):aliasError?$('[data-field="aliases"]'):$('[data-error="timing"]')?.previousElementSibling?.querySelector('input'))?.focus();return false}return true;
  }
  function saveCommand(){if(!validateCommand())return false;const c=clone(state.editor);c.keyword=c.keyword.trim();const index=commands.findIndex(x=>x.id===c.id);if(index===-1)commands.push(c);else commands[index]=c;state.editor=clone(c);state.original=clone(c);updateSaveBar();toast('命令已保存 · 仅在本次预演中生效');return true}
  function addCommand(){guard(()=>{const type=state.page;const isScript=type==='scripts';const used=commands.flatMap(x=>[x.keyword,...x.aliases.split(/[,，]/)]).map(x=>x.trim().toLowerCase());let n=1;while(used.includes(`new${n}`))n++;
    const c={id:`new-${++sequence}`,type,name:isScript?'新脚本命令':'新网页搜索',keyword:`new${n}`,aliases:'',description:'',path:'',enabled:true,icon:isScript?'code':'globe',runtime:isScript?'Python':undefined,run:'enter',delay:50,timeout:10000,hint:'',output:'纯文本',encoding:'UTF-8',action:'复制结果'};
    state.selected=c.id;state.editor=c;state.original=null;
    if(state.direction==='refined'){$('#page-content').innerHTML=editorMarkup()}else renderCommands();
    if(state.direction==='native'){$('#dialog-editor').innerHTML=editorMarkup();$('#editor-dialog').showModal()}updateSaveBar();$('[data-field="name"]')?.focus();
  })}
  function toggleRow(title,desc,key){return `<div class="setting-row"><div><strong>${title}</strong><p>${desc}</p></div><input type="checkbox" class="toggle" data-setting="${key}" aria-label="${title}" ${state.ordinary[key]?'checked':''}></div>`}
  function renderGeneral(){const o=state.ordinary;$('#page-content').innerHTML=`<div class="settings-container">${brandSettings()}<h3 class="section-heading">启动与唤起</h3><section class="settings-group">${toggleRow('开机启动','登录 Windows 后，在后台启动 Suo。','startup')}<div class="setting-row"><div><strong>全局快捷键</strong><p>从任何界面唤起搜索框。</p></div><div class="setting-control"><kbd id="hotkey-value">${esc(o.hotkey)}</kbd><button class="button" data-action="record-hotkey">录制</button></div></div></section><h3 class="section-heading">搜索窗口</h3><section class="settings-group">${toggleRow('失去焦点时收起','点击其他窗口时，自动隐藏搜索框。','closeBlur')}${toggleRow('保留上次输入','再次唤起时，接着上一次的内容搜索。','keepInput')}<div class="setting-row"><div><strong>窗口尺寸</strong><p>使用逻辑像素，随显示器缩放。</p></div><div class="setting-control"><label><span class="sr-only">宽度</span><input type="number" data-setting="width" value="${o.width}" min="560" max="1200" aria-label="窗口宽度"></label><span>×</span><label><input type="number" data-setting="height" value="${o.height}" min="320" max="720" aria-label="窗口高度"></label><span>px</span></div></div></section><h3 class="section-heading">配置与保存</h3><section class="settings-group">${toggleRow('自动保存普通设置','脚本、网页搜索和自定义皮肤仍需明确保存。','auto')}<div class="setting-row path-row"><div><strong>配置文件位置</strong><p>更换位置后，旧文件会保留为恢复副本。</p></div><div class="path-display">%APPDATA%\com.suo.launcher\config.json</div><div class="setting-control"><button class="button" data-action="open-config">${icon('folder')} 打开文件夹</button><button class="button" data-action="relocate">更换位置…</button></div></div></section></div>`}
  function renderSearch(){const o=state.ordinary;$('#page-content').innerHTML=`<div class="settings-container"><h3 class="section-heading">文件搜索</h3><section class="settings-group"><div class="setting-row"><div><strong>Everything</strong><p>优先使用已运行的 Everything 服务。</p></div><span class="subtle-badge">状态示例 · 已连接</span></div><div class="setting-row"><div><strong>搜索顺序</strong><p>已安装 Everything → 安装版内置 Everything → 限定目录索引</p></div></div><div class="setting-row"><div><strong>备用索引</strong><p>仅索引预设的有限目录。</p></div><button class="button" data-action="rebuild">重建索引</button></div></section><h3 class="section-heading">输入响应</h3><section class="settings-group"><div class="setting-row"><div><strong>空输入延迟</strong><p>清空搜索框后，等待多久更新推荐结果。</p></div><label class="setting-control"><input type="number" min="0" max="60000" value="${o.emptyDelay}" data-setting="emptyDelay" aria-label="空输入延迟"><span>ms</span></label></div><div class="setting-row"><div><strong>搜索输入延迟</strong><p>停止输入后，等待多久开始普通搜索。</p></div><label class="setting-control"><input type="number" min="0" max="60000" value="${o.queryDelay}" data-setting="queryDelay" aria-label="搜索输入延迟"><span>ms</span></label></div></section><p class="info-note">${icon('info')}即时脚本使用各自的输入停顿延迟，这里的设置不会覆盖它。</p></div>`}
  function renderTranslation(){const tr=state.translation;$('#page-content').innerHTML=`<div class="settings-container translation-card"><section class="settings-group"><div class="setting-row"><div><strong>翻译服务</strong><p>输入关键词，即可翻译后面的完整文本。</p></div><span class="subtle-badge">示例配置</span></div><div class="editor-body"><div class="form-grid"><label class="field"><span>服务商</span><select data-translation="provider">${['Microsoft Translator','Google Translate','有道翻译'].map(p=>`<option ${p===tr.provider?'selected':''}>${p}</option>`).join('')}</select></label><label class="field"><span>关键词</span><input value="${esc(tr.keyword)}" data-translation="keyword"></label><label class="field wide"><span>默认目标语言</span><select data-translation="target"><option value="en" ${tr.target==='en'?'selected':''}>英语 · en</option><option value="zh-Hans" ${tr.target==='zh-Hans'?'selected':''}>简体中文 · zh-Hans</option></select></label></div><div class="section-gap eyebrow">服务凭据</div><div class="setting-row" style="padding-left:0;padding-right:0"><div><strong>尚未配置凭据</strong><p>凭据独立保存到系统凭据库。</p></div><button class="button" data-action="credentials">配置凭据…</button></div><div class="invocation">${icon('language')}<span>调用示例</span><code>fy 你好，世界</code></div></div><div class="editor-footer"><small>凭据与普通配置分开管理</small><button class="button primary" data-action="save-translation">保存翻译配置</button></div></section></div>`}
  const themeNames={midnight:'午夜',paper:'纸张',forest:'森林',custom:'我的皮肤'};
  function previewMarkup(){const s=state.theme[state.scope],selected=s.selected,selectedCustom=customTheme(),style=selectedCustom?(state.themeDraft?.base||selectedCustom.base||'paper'):selected;
    const inside=state.scope==='launcher'?`<div class="launcher-preview-search">${icon('search')} 设计</div><div class="launcher-result active">${icon('folder')}<span>设计资料<small>文件夹 · 文档</small></span><kbd>↵</kbd></div><div class="launcher-result">${icon('file')}<span>设计规范.pdf<small>PDF 文档 · 项目资料</small></span></div><div class="launcher-result">${icon('globe')}<span>搜索「设计」<small>Google · 网页搜索</small></span></div>`:`<div class="launcher-preview-search">Suo 设置</div><div class="mini-settings"><div class="mini-nav"><div>通用</div><div>脚本命令</div><div class="active">外观</div></div><div class="mini-main"><strong>通用</strong><div class="mini-row"><span>开机启动</span><span>已关闭</span></div><div class="mini-row"><span>全局快捷键</span><span>Alt + Space</span></div><div class="mini-row"><span>自动保存</span><span>已开启</span></div></div></div>`;
    return `<div class="launcher-preview ${style}" id="skin-preview" style="${selectedCustom?`border-radius:${state.themeDraft?.radius||12}px;--preview-selected:${esc(state.themeDraft?.accent||'#365847')};`:''}">${inside}</div>`}
  function selectTheme(id){
    guard(()=>{
      state.theme[state.scope].selected=id;state.themeDraft=null;state.themeOriginal=null;
      if($('#theme-library-dialog').open)$('#theme-library-dialog').close();
      renderAppearance();$('[data-action="open-library"]')?.focus();
    });
  }
  function renderLibraryResults(){
    const current=state.theme[state.scope],all=libraryThemes();
    const matches=all.filter(t=>state.library.filter==='all'||t.kind===state.library.filter).filter(t=>t.name.toLowerCase().includes(state.library.search.toLowerCase()));
    $('#library-count').textContent=`${all.length} 款皮肤 · ${all.filter(t=>t.kind==='builtin').length} 款内置 / ${all.filter(t=>t.kind==='custom').length} 款自定义`;
    $('#library-result-count').textContent=`显示 ${matches.length} / ${all.length} 款`;
    $$('[data-library-filter]').forEach(b=>{b.classList.toggle('active',b.dataset.libraryFilter===state.library.filter);b.setAttribute('aria-pressed',String(b.dataset.libraryFilter===state.library.filter))});
    $('#theme-library-results').innerHTML=matches.map(t=>`<button class="library-item ${current.selected===t.id?'selected':''}" data-library-theme="${t.id}" aria-pressed="${current.selected===t.id}"><span class="library-thumb ${t.base}" aria-hidden="true"><span></span><i></i><i></i></span><span class="library-item-copy"><strong>${esc(t.name)}</strong><small>${t.kind==='builtin'?'内置皮肤':'自定义皮肤'}${current.active===t.id?' · 使用中':''}</small></span>${current.selected===t.id?'<span class="library-selected">'+icon('check')+' 预览中</span>':icon('arrow')}</button>`).join('')||'<div class="empty-state">没有匹配的皮肤。试试其他名称。</div>';
  }
  function openLibrary(){
    const dialog=$('#theme-library-dialog'),styles=getComputedStyle($('#app'));
    for(const key of ['--bg','--panel','--subtle','--text','--muted','--border','--accent','--tint'])dialog.style.setProperty(key,styles.getPropertyValue(key));
    $('#library-scope').textContent=state.scope==='launcher'?'搜索界面':'设置界面';
    state.library={filter:'all',search:''};$('#theme-library-search').value='';renderLibraryResults();
    dialog.showModal();$('#theme-library-results').scrollTop=0;$('#theme-library-search').focus();
  }
  function renderAppearance(){
    const s=state.theme[state.scope],selectedCustom=customTheme(),custom=!!selectedCustom;
    if(custom&&!state.themeDraft){state.themeDraft=clone(selectedCustom);state.themeOriginal=clone(selectedCustom)}
    const selectedName=themeName(s.selected),previewBase=selectedCustom?.base||s.selected;
    $('#page-content').innerHTML=`
      <div class="appearance-top"><div class="scope-switch" aria-label="外观作用范围"><button data-scope="launcher" class="${state.scope==='launcher'?'active':''}" aria-pressed="${state.scope==='launcher'}">搜索界面</button><button data-scope="settings" class="${state.scope==='settings'?'active':''}" aria-pressed="${state.scope==='settings'}">设置界面</button></div><div class="current-theme"><span class="local-dot"></span> 当前使用：${esc(themeName(s.active))}</div></div>
      <section class="theme-chooser" aria-label="当前选择的皮肤">
        <span class="library-thumb ${previewBase}" aria-hidden="true"><span></span><i></i><i></i></span>
        <div class="theme-chooser-copy"><small>正在预览</small><strong>${esc(selectedName)} <span>${custom?'自定义':'内置'}</span></strong></div>
        <div class="theme-chooser-actions"><span>${libraryThemes().length} 款皮肤</span><button class="button" data-action="open-library" aria-haspopup="dialog">更换皮肤 ${icon('arrow')}</button></div>
      </section>
      <div class="appearance-grid compact-library-layout">
        <section class="theme-editor"><header class="theme-edit-heading"><div><h3>${custom?'编辑皮肤':'皮肤设置'}</h3><p>${custom?'修改只更新预览，保存后可应用。':'内置皮肤不可覆盖，可复制一份自定义。'}</p><strong class="editing-theme-name">${esc(selectedName)}</strong></div><span class="subtle-badge">${custom?'可编辑':'内置只读'}</span></header>
        ${!custom?'<div class="builtin-theme-state">'+icon('palette')+'<strong>从熟悉的样子开始</strong><p>保留这款皮肤的颜色与结构，再调整字号、圆角等细节。</p><button class="button primary" data-action="customize">'+icon('plus')+' 复制并自定义</button></div>':`
          <div class="theme-control"><span>强调色</span><div class="swatches">${['#365847','#3c5482','#684b83','#805743'].map(color=>`<button class="swatch" style="--swatch:${color}" data-accent="${color}" aria-label="强调色 ${color}" aria-pressed="${state.themeDraft.accent===color}"></button>`).join('')}</div></div>
          <label class="theme-control"><span>窗口圆角</span><div><input type="range" min="0" max="24" value="${state.themeDraft.radius}" data-theme-setting="radius"><output>${state.themeDraft.radius} px</output></div></label>
          <label class="theme-control"><span>基础字号</span><div><input type="range" min="12" max="20" value="${state.themeDraft.fontSize}" data-theme-setting="fontSize"><output>${state.themeDraft.fontSize} px</output></div></label>
          <div class="theme-actions"><button class="button" data-action="discard-theme">还原</button><button class="button primary" data-action="save-theme">保存皮肤</button></div>`}
        </section>
        <aside class="preview-side"><div class="preview-label"><span>预览 · ${esc(selectedName)}</span><span>尚未应用</span></div><div class="preview-canvas">${previewMarkup()}</div><p class="theme-status">当前使用：<strong>${esc(themeName(s.active))}</strong><br>${s.active===s.selected?'预览的是正在使用的皮肤。':'应用后才会改变'+(state.scope==='launcher'?'搜索':'设置')+'界面。'}</p><div class="theme-actions"><button class="button primary" data-action="apply-theme" ${s.selected===s.active||themeDirty()?'disabled':''}>${icon('check')} ${s.selected===s.active?'当前正在使用':'应用这款皮肤'}</button></div></aside>
      </div>`;
    updateThemePreview();updateSaveBar();
  }
  function addPrecisionControls(){
    $$('[data-theme-setting]').filter(e=>e.type==='range').forEach(slider=>{
      if(slider.closest('.theme-control').querySelector('.precision-input'))return;
      const key=slider.dataset.themeSetting,label=key==='fontSize'?'基础字号':'窗口圆角';
      const output=slider.nextElementSibling;
      output.outerHTML=`<span class="precision-control"><input class="precision-input" type="number" min="${key==='fontSize'?1:0}" max="255" step="1" value="${state.themeDraft[key]}" data-theme-setting="${key}" aria-label="${label}精确数值"><span>px</span></span>`;
      slider.setAttribute('aria-label',label+'常用范围');
      slider.closest('.theme-control').insertAdjacentHTML('afterend',`<p class="range-note" data-range-note="${key}">滑块是常用范围；也可直接输入 ${key==='fontSize'?'1':'0'}–255 px。</p>`);
    });
    if(state.themeDraft&&!$('#range-feedback'))$('.theme-editor .theme-actions')?.insertAdjacentHTML('beforebegin','<p class="range-feedback" id="range-feedback" role="status"></p>');
  }
  function updateThemePreview(){
    addPrecisionControls();
    const p=$('#skin-preview');
    if(p&&customTheme()&&state.themeDraft){
      p.classList.add('custom-preview');
      const size=state.themeDraft.fontSize;p.style.borderRadius=state.themeDraft.radius+'px';p.style.setProperty('--preview-selected',state.themeDraft.accent);
      $$('.launcher-result,.launcher-preview-search,.mini-main strong,.mini-row,.mini-nav',p).forEach(x=>x.style.fontSize=size+'px');
      $$('.launcher-result small',p).forEach(x=>x.style.fontSize=Math.max(1,size-2)+'px');
      const hint=$('#range-feedback');if(hint)hint.textContent=size>20?`已超出常用范围，仍可保存。${size} px 下请检查换行与控件空间。`:size<12?`已超出常用范围，仍可保存。${size} px 的文字可能较难阅读。`:'常用范围 12–20 px；精确输入可以突破滑块范围。';
    }
    if(p){
      const canvas=p.closest('.preview-canvas');let frame=canvas.querySelector('.preview-frame');
      if(!frame){frame=document.createElement('div');frame.className='preview-frame';p.replaceWith(frame);frame.append(p)}
      const scale=Math.min(1,(canvas.clientWidth-36)/600);p.style.transform=`scale(${scale})`;frame.style.width=(600*scale)+'px';frame.style.height=(p.offsetHeight*scale)+'px';
      const label=$('.preview-label span:last-child');if(label)label.textContent=`局部预览 · ${Math.round(scale*100)}%`;
    }
    const b=$('[data-action="apply-theme"]');if(b)b.disabled=state.theme[state.scope].selected===state.theme[state.scope].active||themeDirty();
  }
  function saveTheme(){if($('[data-theme-setting][aria-invalid="true"]')){toast('请先修正高亮的数值。');return false}const scope=state.theme[state.scope];if(scope.selected==='custom')scope.custom=clone(state.themeDraft);else scope.customThemes=scope.customThemes.map(t=>t.id===scope.selected?clone(state.themeDraft):t);state.themeOriginal=clone(state.themeDraft);updateSaveBar();updateThemePreview();toast('皮肤已保存，使用中的皮肤保持原样 · 演示');return true}
  document.addEventListener('input',event=>{
    const e=event.target;
    if(e.id==='theme-library-search'){state.library.search=e.value;renderLibraryResults();return}
    if(e.id==='command-search'){const caret=e.selectionStart;state.search=e.value;renderCommands();const n=$('#command-search');n.focus();if(n.type!=='search')n.setSelectionRange(caret,caret);return}
    if(e.dataset.field&&state.editor){const key=e.dataset.field;state.editor[key]=e.type==='checkbox'?e.checked:e.type==='number'?(e.value===''?'':Number(e.value)):e.value;if(key==='keyword'){const text=$('#invocation-code');if(text)text.textContent=e.value+(state.editor.type==='scripts'?' 1727433600':' 搜索内容')}if(key==='action')$$('[data-shell-warning]').forEach(x=>x.hidden=e.value!=='执行 Shell 命令');if(['keyword','aliases','delay','timeout'].includes(key))validateCommand();updateSaveBar()}
    if(e.dataset.translation){state.translation[e.dataset.translation]=e.value;updateSaveBar()}
    if(e.dataset.themeSetting){
      const key=e.dataset.themeSetting,value=Number(e.value),minimum=key==='fontSize'?1:0;
      if(e.value===''||!Number.isInteger(value)||value<minimum||value>255){e.setAttribute('aria-invalid','true');const f=$('#range-feedback');if(f)f.textContent=`请输入 ${minimum}–255 的整数，超出范围的输入尚未保存。`;return}
      e.removeAttribute('aria-invalid');state.themeDraft[key]=value;
      $$(`[data-theme-setting="${key}"]`).forEach(input=>{if(input!==e)input.value=input.type==='range'?Math.min(Number(input.max),Math.max(Number(input.min),value)):value});
      updateThemePreview();updateSaveBar();
    }
  });
  document.addEventListener('change',event=>{
    const e=event.target;
    if(e.dataset.setting){const key=e.dataset.setting;if(e.type==='number'){const value=Number(e.value);if(e.value===''||!Number.isInteger(value)||value<Number(e.min)||value>Number(e.max)){e.setAttribute('aria-invalid','true');e.setCustomValidity(`请输入 ${e.min}–${e.max} 的整数`);e.reportValidity();return}e.setCustomValidity('');e.removeAttribute('aria-invalid')}
      const value=e.type==='checkbox'?e.checked:Number(e.value);if(key==='auto'&&value&&ordinaryDirty()){e.checked=false;guard(()=>{state.ordinary.auto=true;state.ordinarySaved=clone(state.ordinary);renderGeneral();updateSaveBar()},true);return}state.ordinary[key]=value;if(state.ordinary.auto||key==='auto')state.ordinarySaved=clone(state.ordinary);updateSaveBar()}
    if(e.dataset.enable){const c=commands.find(c=>c.id===e.dataset.enable);if(c)c.enabled=e.checked;toast(e.checked?'命令已启用 · 演示':'命令已停用 · 演示')}
  });
  document.addEventListener('click',event=>{
    const b=event.target.closest('button');if(!b||b.disabled)return;
    if(b.dataset.brandStyle){state.ordinary.iconStyle=b.dataset.brandStyle;if(state.ordinary.auto)state.ordinarySaved=clone(state.ordinary);updateBrand();updateSaveBar();return}
    if(b.dataset.libraryFilter){state.library.filter=b.dataset.libraryFilter;renderLibraryResults();return}
    if(b.dataset.libraryTheme){selectTheme(b.dataset.libraryTheme);return}
    if(b.dataset.direction){guard(()=>{if($('#editor-dialog').open)$('#editor-dialog').close();state.direction=b.dataset.direction;state.editor=null;state.original=null;render()});return}
    if(b.dataset.page){navigate(b.dataset.page);return}
    if(b.dataset.command){if(b.dataset.command===state.selected&&state.direction!=='native')return;guard(()=>openEditor(b.dataset.command));return}
    if(b.dataset.filter){state.filter=b.dataset.filter;renderCommands();return}
    if(b.dataset.run){state.editor.run=b.dataset.run;const root=state.direction==='native'?$('#dialog-editor'):state.direction==='studio'?$('#inline-editor'):$(`.accordion-card.selected`);if(state.direction==='refined')renderCommands();else root.innerHTML=editorMarkup();updateSaveBar();return}
    if(b.dataset.scope){guard(()=>{state.scope=b.dataset.scope;state.themeDraft=null;state.themeOriginal=null;renderAppearance()});return}
    if(b.dataset.theme){selectTheme(b.dataset.theme);return}
    if(b.dataset.accent){state.themeDraft.accent=b.dataset.accent;$$('[data-accent]').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));updateThemePreview();updateSaveBar();return}
    const action=b.dataset.action;
    if(action==='open-library')openLibrary();
    if(action==='close-library'){$('#theme-library-dialog').close();$('[data-action="open-library"]')?.focus()}
    if(action==='add')addCommand();
    if(action==='save-command'){if(saveCommand()){if(state.direction==='native'){$('#editor-dialog').close();state.editor=null;state.original=null}renderCommands()}}
    if(action==='discard-command'){guard(()=>{state.editor=null;state.original=null;if($('#editor-dialog').open)$('#editor-dialog').close();renderCommands()})}
    if(action==='save-general'){state.ordinarySaved=clone(state.ordinary);updateSaveBar();toast('设置已保存 · 演示')}
    if(action==='save-translation'){const kw=state.translation.keyword.trim().toLowerCase();const used=commands.flatMap(c=>[c.keyword,...c.aliases.split(/[,，]/)]).map(x=>x.trim().toLowerCase());if(!kw||/\s/.test(kw)||used.includes(kw)){notice('翻译关键词为空、包含空格或已被其他命令占用。');return}state.translationSaved=clone(state.translation);updateSaveBar();toast('翻译配置已保存 · 演示')}
    if(action==='customize'){const s=state.theme[state.scope];if(libraryThemes().filter(t=>t.kind==='custom').length>=12){notice('每个界面最多保存 12 款自定义皮肤。请先编辑现有皮肤。');return}if(s.custom){s.customThemes.push({...s.custom,id:`copy-${++sequence}`,name:'我的皮肤 '+sequence})}s.custom={base:s.selected,accent:'#365847',radius:12,fontSize:14};s.selected='custom';state.themeDraft=clone(s.custom);state.themeOriginal=null;renderAppearance()}
    if(action==='save-theme')saveTheme();
    if(action==='discard-theme'){guard(()=>{state.themeDraft=clone(state.themeOriginal);if(!state.themeDraft){state.theme[state.scope].selected='paper';state.theme[state.scope].custom=null}renderAppearance()})}
    if(action==='apply-theme'){state.theme[state.scope].active=state.theme[state.scope].selected;renderAppearance();toast('皮肤已应用到'+(state.scope==='launcher'?'搜索界面':'设置界面')+' · 演示')}
    if(action==='record-hotkey'){
      b.textContent='请按组合键…';const old=state.ordinary.hotkey;
      const handler=e=>{e.preventDefault();e.stopImmediatePropagation();if(e.key==='Escape'){b.textContent='录制';document.removeEventListener('keydown',handler,true);return}if(['Control','Alt','Shift','Meta'].includes(e.key))return;if(!e.ctrlKey&&!e.altKey&&!e.metaKey){toast('请包含 Ctrl、Alt 或 Win 修饰键');return}const key=[e.ctrlKey?'Ctrl':'',e.altKey?'Alt':'',e.shiftKey?'Shift':'',e.metaKey?'Win':'',e.code==='Space'?'Space':e.key.toUpperCase()].filter(Boolean).join(' + ');state.ordinary.hotkey=key;if(state.ordinary.auto)state.ordinarySaved=clone(state.ordinary);$('#hotkey-value').textContent=key;b.textContent='录制';document.removeEventListener('keydown',handler,true);updateSaveBar();toast('组合键预演完成；原生注册与冲突检测待接入')};document.addEventListener('keydown',handler,true);
      b.addEventListener('blur',()=>{document.removeEventListener('keydown',handler,true);b.textContent='录制'}, {once:true});
    }
    const demoNotices={'pick-file':'正式界面将在这里打开系统文件选择器。本稿展示路径输入与运行环境选择，不会访问或执行本地脚本。','pick-icon':'正式界面将选择本地图标，并校验文件类型、大小和尺寸。本稿不导入文件。','open-config':'正式界面将在资源管理器中打开实际配置目录。本稿显示的是占位路径。','relocate':'正式界面将先验证新目录，再迁移配置并保留旧文件。当前只是流程示例，不移动文件。','rebuild':'正式界面将在这里显示索引进度和完成状态。本稿不会扫描磁盘。','credentials':'正式界面将打开独立凭据面板，通过系统凭据库存储密钥。本稿不接收真实密钥。'};
    if(demoNotices[action])notice(demoNotices[action]);
  });
  $('#guard-stay').onclick=()=>{$('#guard-dialog').close();pendingAction=null};
  $('#guard-discard').onclick=()=>{discardCurrent();$('#guard-dialog').close();const action=pendingAction;pendingAction=null;action?.()};
  $('#guard-save').onclick=()=>{if(!saveCurrent()){$('#guard-dialog').close();pendingAction=null;toast('请先修正表单中的错误。');return}$('#guard-dialog').close();const action=pendingAction;pendingAction=null;action?.()};
  $('#notice-close').onclick=()=>$('#notice-dialog').close();
  $('#save-preference').onclick=()=>{navigate('general')};
  $('#close-window').onclick=()=>guard(()=>notice('关闭流程预演完成。正式应用会隐藏设置窗口；此处保留设计稿供继续比较。'),true);
  $('#editor-dialog').addEventListener('cancel',e=>{e.preventDefault();guard(()=>{$('#editor-dialog').close();state.editor=null;state.original=null;updateSaveBar()})});
  $('#guard-dialog').addEventListener('cancel',()=>{pendingAction=null});
  window.addEventListener('beforeunload',e=>{if(anyDirty()){e.preventDefault();e.returnValue=''}});
  window.addEventListener('resize',()=>{if(state.page==='appearance')updateThemePreview()});
  render();
})();
