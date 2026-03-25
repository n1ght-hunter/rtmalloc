(function () {
  "use strict";

  function init() {
    var charts = document.querySelectorAll(".uplot-chart");
    if (charts.length === 0) return;
    charts.forEach(initChart);
  }

  function initChart(container) {
    var dataEl = container.querySelector("script.uplot-data");
    if (!dataEl) return;

    var chartJson;
    try {
      chartJson = JSON.parse(dataEl.textContent);
    } catch (e) {
      container.innerHTML = "<p>Failed to parse chart data: " + e.message + "</p>";
      return;
    }

    if (!chartJson.labels || !chartJson.datasets) return;

    var editable = chartJson.editable === true;
    var codeOpt = chartJson.code;
    var codeCfg = null;
    if (codeOpt === false && !editable) {
      codeCfg = null;
    } else if (codeOpt && typeof codeOpt === "object") {
      codeCfg = {
        position: codeOpt.position || "below",
        collapsible: codeOpt.collapsible !== false,
        open: codeOpt.open === true
      };
    } else {
      codeCfg = { position: null, collapsible: true, open: false };
    }

    var chartView = document.createElement("div");
    chartView.className = "uplot-view";
    container.appendChild(chartView);

    var state = createChart(chartView, chartJson);

    if (editable) {
      var toolbar = document.createElement("div");
      toolbar.className = "uplot-toolbar";
      container.insertBefore(toolbar, chartView);

      var panels = [];
      function addTab(label, el) {
        var btn = document.createElement("button");
        btn.textContent = label;
        toolbar.appendChild(btn);
        el.style.display = "none";
        container.appendChild(el);
        panels.push({ btn: btn, el: el });
        btn.addEventListener("click", function () {
          for (var j = 0; j < panels.length; j++) {
            panels[j].el.style.display = "none";
            panels[j].btn.className = "";
          }
          el.style.display = "";
          btn.className = "active";
          if (el === chartView) state.resize();
        });
        return btn;
      }

      container.removeChild(chartView);
      var btnChart = addTab("Chart", chartView);

      var editWrap = document.createElement("div");
      editWrap.className = "uplot-editor-wrap";

      var editorBox = document.createElement("div");
      editorBox.className = "uplot-editor";

      var pre = document.createElement("pre");
      pre.className = "uplot-editor-highlight";

      var textarea = document.createElement("textarea");
      textarea.className = "uplot-editor-input";
      var jsonStr = JSON.stringify(chartJson, null, 2);
      textarea.value = jsonStr;
      textarea.spellcheck = false;

      editorBox.appendChild(pre);
      editorBox.appendChild(textarea);
      editWrap.appendChild(editorBox);

      var statusBar = document.createElement("div");
      statusBar.className = "uplot-editor-status";

      var errorEl = document.createElement("div");
      errorEl.className = "uplot-edit-error";
      errorEl.style.display = "none";
      statusBar.appendChild(errorEl);

      var resetBtn = document.createElement("button");
      resetBtn.className = "uplot-reset-btn";
      resetBtn.textContent = "Reset";
      statusBar.appendChild(resetBtn);

      editWrap.appendChild(statusBar);

      function syncHighlight() {
        pre.innerHTML = syntaxHighlight(textarea.value) + "\n";
      }

      function syncSize() {
        textarea.style.height = "auto";
        var h = Math.max(textarea.scrollHeight, 120);
        textarea.style.height = h + "px";
        pre.style.height = h + "px";
      }

      function syncScroll() {
        pre.scrollTop = textarea.scrollTop;
        pre.scrollLeft = textarea.scrollLeft;
      }

      syncHighlight();
      setTimeout(syncSize, 0);

      textarea.addEventListener("scroll", syncScroll);

      textarea.addEventListener("keydown", function (e) {
        if (e.key === "Tab") {
          e.preventDefault();
          var s = textarea.selectionStart;
          var end = textarea.selectionEnd;
          textarea.value = textarea.value.substring(0, s) + "  " + textarea.value.substring(end);
          textarea.selectionStart = textarea.selectionEnd = s + 2;
          textarea.dispatchEvent(new Event("input"));
        }
      });

      var timer = null;
      textarea.addEventListener("input", function () {
        syncHighlight();
        syncSize();
        clearTimeout(timer);
        timer = setTimeout(function () {
          var newJson;
          try {
            newJson = JSON.parse(textarea.value);
          } catch (e) {
            errorEl.textContent = e.message;
            errorEl.style.display = "block";
            return;
          }
          errorEl.style.display = "none";
          if (!newJson.labels || !newJson.datasets) return;
          state.destroy();
          chartView.innerHTML = "";
          state = createChart(chartView, newJson);
        }, 400);
      });

      resetBtn.addEventListener("click", function () {
        textarea.value = jsonStr;
        syncHighlight();
        syncSize();
        errorEl.style.display = "none";
        state.destroy();
        chartView.innerHTML = "";
        state = createChart(chartView, chartJson);
      });

      addTab("Edit", editWrap);
      btnChart.className = "active";
      chartView.style.display = "";
      return;
    }

    if (!codeCfg) return;

    var codeEl = document.createElement("div");
    codeEl.className = "uplot-code-block";
    codeEl.appendChild(jsonTree(chartJson, true));

    var pos = codeCfg.position;

    if (pos === "side") {
      var sideWrap = document.createElement("div");
      sideWrap.className = "uplot-side";
      container.removeChild(chartView);

      var sideLeft = document.createElement("div");
      sideLeft.className = "uplot-side-chart";
      sideLeft.appendChild(chartView);

      var sideRight = document.createElement("div");
      sideRight.className = "uplot-side-code";
      sideRight.appendChild(codeEl);

      sideWrap.appendChild(sideLeft);
      sideWrap.appendChild(sideRight);
      container.appendChild(sideWrap);
      setTimeout(function () { state.resize(); }, 0);
    } else if (pos === "above" || pos === "below") {
      if (codeCfg.collapsible) {
        var details = document.createElement("details");
        if (codeCfg.open) details.open = true;
        var summary = document.createElement("summary");
        summary.textContent = "Show code";
        details.appendChild(summary);
        details.appendChild(codeEl);
        if (pos === "above") {
          container.insertBefore(details, chartView);
        } else {
          container.appendChild(details);
        }
      } else {
        if (pos === "above") {
          container.insertBefore(codeEl, chartView);
        } else {
          container.appendChild(codeEl);
        }
      }
    } else {
      var toolbar = document.createElement("div");
      toolbar.className = "uplot-toolbar";
      container.insertBefore(toolbar, chartView);

      var panels = [];
      function addCodeTab(label, el) {
        var btn = document.createElement("button");
        btn.textContent = label;
        toolbar.appendChild(btn);
        el.style.display = "none";
        container.appendChild(el);
        panels.push({ btn: btn, el: el });
        btn.addEventListener("click", function () {
          for (var j = 0; j < panels.length; j++) {
            panels[j].el.style.display = "none";
            panels[j].btn.className = "";
          }
          el.style.display = "";
          btn.className = "active";
          if (el === chartView) state.resize();
        });
        return btn;
      }

      container.removeChild(chartView);
      var btnChart = addCodeTab("Chart", chartView);
      addCodeTab("Code", codeEl);
      btnChart.className = "active";
      chartView.style.display = "";
    }
  }

  function fmtVal(val, fmt) {
    if (!fmt) return val.toFixed(2);
    var s = val.toFixed(fmt.decimals != null ? fmt.decimals : 2);
    if (fmt.prefix) s = fmt.prefix + s;
    if (fmt.suffix) s = s + fmt.suffix;
    return s;
  }

  function createChart(container, json) {
    var type = json.type || "bar";
    var labels = json.labels || [];
    var datasets = json.datasets || [];
    var title = json.title || "";
    var xLabel = (json.axes && json.axes.x) || "";
    var yLabel = (json.axes && json.axes.y) || "";
    var globalFmt = json.format || null;

    if (labels.length === 0 || datasets.length === 0) {
      container.innerHTML = "<p><em>No data available.</em></p>";
      return { destroy: function () {}, resize: function () {} };
    }

    var hasStringLabels = typeof labels[0] === "string";
    var xValues = hasStringLabels
      ? labels.map(function (_, i) { return i; })
      : labels;

    var uData = type === "bar" ? [labels] : [xValues];
    var uSeries = [{ label: xLabel || "x" }];

    var dsFmts = [];
    for (var i = 0; i < datasets.length; i++) {
      var ds = datasets[i];
      var color = ds.color || nextColor(i);
      uData.push(ds.data || labels.map(function () { return null; }));
      uSeries.push(buildSeries(ds, color, type, i));
      dsFmts.push(ds.format || globalFmt);
    }

    var theme = detectTheme();
    var height = json.height || 350;

    var xAxis = { stroke: theme.text, label: xLabel };
    if (type !== "bar" && hasStringLabels) {
      xAxis.values = function (u, splits) {
        return splits.map(function (v) {
          var idx = Math.round(v);
          return idx >= 0 && idx < labels.length ? labels[idx] : "";
        });
      };
      xAxis.space = function () { return 80; };
    }

    var yAxis = {
      stroke: theme.text,
      label: yLabel,
      ticks: { stroke: theme.grid },
      grid: { stroke: theme.grid },
    };
    if (globalFmt) {
      yAxis.values = function (u, splits) {
        return splits.map(function (v) { return fmtVal(v, globalFmt); });
      };
    }

    var tooltipCfg = json.tooltip;
    var showTooltip = tooltipCfg !== false;

    var opts = {
      title: title,
      width: container.clientWidth || 800,
      height: height,
      legend: json.legend || { live: false },
      scales: type !== "bar" ? { x: { time: false } } : {},
      axes: [xAxis, yAxis],
      series: uSeries,
      plugins: showTooltip ? [tooltipPlugin(json, type === "bar" ? labels : null, dsFmts)] : [],
    };

    opts.cursor = { x: false, y: false };

    if (type === "bar") {
      opts.plugins.unshift(seriesBarsPlugin({ ori: 0, dir: 1, radius: 0.3 }));
    }

    if (type === "scatter") {
      opts.cursor.points = { show: false };
    }

    if (json.opts) {
      opts = deepMerge(opts, json.opts);
    }

    var chart = new uPlot(opts, uData, container);

    var ro = new ResizeObserver(function (entries) {
      for (var j = 0; j < entries.length; j++) {
        var w = entries[j].contentRect.width;
        if (w > 0) chart.setSize({ width: w, height: height });
      }
    });
    ro.observe(container);

    return {
      destroy: function () {
        ro.disconnect();
        chart.destroy();
      },
      resize: function () {
        var w = container.clientWidth;
        if (w > 0) chart.setSize({ width: w, height: height });
      },
    };
  }

  function buildSeries(ds, color, type, index) {
    var s = { label: ds.label || "series" };

    switch (type) {
      case "line":
        s.stroke = color;
        s.width = ds.width || 2;
        s.fill = ds.fill || null;
        if (ds.dash) s.dash = ds.dash;
        if (ds.points) s.points = { show: true, size: 4 };
        break;

      case "area":
        s.stroke = color;
        s.width = ds.width || 2;
        s.fill = ds.fill || colorAlpha(color, 0.3);
        if (ds.dash) s.dash = ds.dash;
        if (ds.points) s.points = { show: true, size: 4 };
        break;

      case "scatter":
        s.stroke = color;
        s.fill = color;
        s.paths = function () { return null; };
        s.points = {
          show: true,
          size: ds.pointSize || 6,
          fill: color,
        };
        break;

      case "bar":
      default:
        s.fill = color;
        s.stroke = color;
        s.width = 0;
        break;
    }

    return s;
  }

  function colorAlpha(hex, alpha) {
    if (hex.charAt(0) !== "#" || hex.length < 7) return hex;
    var r = parseInt(hex.slice(1, 3), 16);
    var g = parseInt(hex.slice(3, 5), 16);
    var b = parseInt(hex.slice(5, 7), 16);
    return "rgba(" + r + "," + g + "," + b + "," + alpha + ")";
  }

  function deepMerge(target, source) {
    var out = {};
    for (var k in target) {
      if (target.hasOwnProperty(k)) out[k] = target[k];
    }
    for (var k in source) {
      if (!source.hasOwnProperty(k)) continue;
      if (
        out[k] && typeof out[k] === "object" && !Array.isArray(out[k]) &&
        source[k] && typeof source[k] === "object" && !Array.isArray(source[k])
      ) {
        out[k] = deepMerge(out[k], source[k]);
      } else {
        out[k] = source[k];
      }
    }
    return out;
  }

  function tooltipPlugin(json, barLabels, dsFmts) {
    var tooltipEl;
    var showAll = json.tooltip && json.tooltip.all;

    function initHook(u) {
      tooltipEl = document.createElement("div");
      tooltipEl.className = "uplot-tooltip";
      tooltipEl.style.display = "none";
      u.over.appendChild(tooltipEl);
      u.over.addEventListener("mouseleave", function () {
        tooltipEl.style.display = "none";
      });
    }

    function resolveLabel(u, idx) {
      if (barLabels) return barLabels[idx];
      return json.labels[idx];
    }

    function setCursor(u) {
      if (showAll) {
        setCursorAll(u);
      } else {
        setCursorSingle(u);
      }
    }

    function setCursorSingle(u) {
      var found = false;
      for (var i = 1; i < u.series.length; i++) {
        var idx = u.cursor.idxs && u.cursor.idxs[i];
        if (idx != null) {
          var s = u.series[i];
          var val = u.data[i][idx];
          if (val != null && s.show) {
            var color = s.fill || s.stroke;
            tooltipEl.innerHTML =
              "<strong>" + esc(String(resolveLabel(u, idx))) + "</strong><br>" +
              '<span style="color:' + color + '">■</span> ' +
              esc(s.label) + ": " + fmtVal(val, dsFmts[i - 1]);
            positionTooltip(u, i, idx);
            found = true;
            break;
          }
        }
      }
      if (!found) tooltipEl.style.display = "none";
    }

    function setCursorAll(u) {
      var lines = [];
      var headerIdx = null;
      var headerSi = null;
      for (var i = 1; i < u.series.length; i++) {
        var idx = u.cursor.idxs && u.cursor.idxs[i];
        if (idx != null) {
          var s = u.series[i];
          var val = u.data[i][idx];
          if (headerIdx == null) { headerIdx = idx; headerSi = i; }
          if (val != null && s.show) {
            var color = s.fill || s.stroke;
            lines.push(
              '<span style="color:' + color + '">■</span> ' +
              esc(s.label) + ": " + fmtVal(val, dsFmts[i - 1])
            );
          }
        }
      }
      if (lines.length > 0) {
        tooltipEl.innerHTML =
          "<strong>" + esc(String(resolveLabel(u, headerIdx))) + "</strong><br>" +
          lines.join("<br>");
        positionTooltip(u, headerSi, headerIdx);
      } else {
        tooltipEl.style.display = "none";
      }
    }

    function positionTooltip(u, seriesIdx, dataIdx) {
      tooltipEl.style.display = "block";

      var left, top;
      if (barLabels) {
        left = u.cursor.left;
        top = u.cursor.top;
      } else {
        left = u.valToPos(u.data[0][dataIdx], "x");
        top = u.valToPos(u.data[seriesIdx][dataIdx], u.series[seriesIdx].scale || "y");
      }

      tooltipEl.style.left = (left + 15) + "px";
      tooltipEl.style.top = (top - 10) + "px";

      var overRect = u.over.getBoundingClientRect();
      var tipRect = tooltipEl.getBoundingClientRect();
      if (tipRect.right > overRect.right)
        tooltipEl.style.left = (left - tipRect.width - 10) + "px";
      if (tipRect.bottom > overRect.bottom)
        tooltipEl.style.top = (top - tipRect.height - 10) + "px";
    }

    return { hooks: { init: initHook, setCursor: setCursor } };
  }

  function detectTheme() {
    var el = document.documentElement;
    var isDark = el.classList.contains("coal")
      || el.classList.contains("navy")
      || el.classList.contains("ayu");
    return {
      text: isDark ? "#c8c9db" : "#333",
      grid: isDark ? "rgba(255,255,255,0.1)" : "rgba(0,0,0,0.1)",
    };
  }

  var PALETTE = [
    "#2ca02c", "#1f77b4", "#ff7f0e", "#d62728", "#9467bd",
    "#17becf", "#e377c2", "#bcbd22", "#98df8a", "#888888",
  ];

  function nextColor(i) { return PALETTE[i % PALETTE.length]; }
  function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }

  function jsonTree(val, expanded) {
    if (val === null) return spanCls("jt-null", "null");
    if (typeof val === "boolean") return spanCls("jt-bool", String(val));
    if (typeof val === "number") return spanCls("jt-num", String(val));
    if (typeof val === "string") return spanCls("jt-str", '"' + esc(val) + '"');

    var isArr = Array.isArray(val);
    var keys = isArr ? null : Object.keys(val);
    var len = isArr ? val.length : keys.length;

    var wrap = document.createElement("span");
    wrap.className = "jt-node";

    var toggle = document.createElement("span");
    toggle.className = "jt-toggle";
    toggle.textContent = expanded ? "\u25BC " : "\u25B6 ";
    wrap.appendChild(toggle);

    var bracket = document.createElement("span");
    bracket.className = "jt-bracket";
    bracket.textContent = isArr ? "[" : "{";
    wrap.appendChild(bracket);

    var preview = document.createElement("span");
    preview.className = "jt-preview";
    preview.textContent = " " + len + (isArr ? " items" : " keys") + " ";
    wrap.appendChild(preview);

    var children = document.createElement("div");
    children.className = "jt-children";
    children.style.display = expanded ? "" : "none";
    preview.style.display = expanded ? "none" : "";

    var entries = isArr ? val : keys;
    for (var i = 0; i < entries.length; i++) {
      var row = document.createElement("div");
      row.className = "jt-row";
      if (!isArr) {
        var k = document.createElement("span");
        k.className = "jt-key";
        k.textContent = '"' + entries[i] + '"';
        row.appendChild(k);
        row.appendChild(document.createTextNode(": "));
      }
      var child = isArr ? val[i] : val[entries[i]];
      var childExpanded = expanded && (isArr ? false : true);
      row.appendChild(jsonTree(child, childExpanded));
      if (i < entries.length - 1) row.appendChild(document.createTextNode(","));
      children.appendChild(row);
    }
    wrap.appendChild(children);

    var closeBracket = document.createElement("span");
    closeBracket.className = "jt-bracket";
    closeBracket.textContent = isArr ? "]" : "}";
    wrap.appendChild(closeBracket);

    toggle.addEventListener("click", function () {
      var open = children.style.display !== "none";
      children.style.display = open ? "none" : "";
      preview.style.display = open ? "" : "none";
      toggle.textContent = open ? "\u25B6 " : "\u25BC ";
    });

    return wrap;
  }

  function spanCls(cls, text) {
    var s = document.createElement("span");
    s.className = cls;
    s.textContent = text;
    return s;
  }

  function syntaxHighlight(json) {
    var escaped = json.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    return escaped.replace(
      /("(?:\\.|[^"\\])*")\s*:/g,
      '<span class="jt-key">$1</span>:'
    ).replace(
      /:\s*("(?:\\.|[^"\\])*")/g,
      function (m, str) { return ': <span class="jt-str">' + str + "</span>"; }
    ).replace(
      /:\s*(-?\d+\.?\d*(?:e[+-]?\d+)?)/gi,
      ': <span class="jt-num">$1</span>'
    ).replace(
      /:\s*(true|false)/g,
      ': <span class="jt-bool">$1</span>'
    ).replace(
      /:\s*(null)/g,
      ': <span class="jt-null">$1</span>'
    );
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
