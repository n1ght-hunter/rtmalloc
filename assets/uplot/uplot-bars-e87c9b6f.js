/// seriesBarsPlugin and dependencies from uPlot demos.
/// Source: https://github.com/leeoniya/uPlot/tree/master/demos
/// License: MIT

function roundDec(val, dec) {
  return Math.round(val * (dec = 10**dec)) / dec;
}

const SPACE_BETWEEN = 1;
const SPACE_AROUND  = 2;
const SPACE_EVENLY  = 3;

const coord = (i, offs, iwid, gap) => roundDec(offs + i * (iwid + gap), 6);

function distr(numItems, sizeFactor, justify, onlyIdx, each) {
  let space = 1 - sizeFactor;

  let gap =  (
    justify == SPACE_BETWEEN ? space / (numItems - 1) :
    justify == SPACE_AROUND  ? space / (numItems    ) :
    justify == SPACE_EVENLY  ? space / (numItems + 1) : 0
  );

  if (isNaN(gap) || gap == Infinity)
    gap = 0;

  let offs = (
    justify == SPACE_BETWEEN ? 0       :
    justify == SPACE_AROUND  ? gap / 2 :
    justify == SPACE_EVENLY  ? gap     : 0
  );

  let iwid = sizeFactor / numItems;
  let _iwid = roundDec(iwid, 6);

  if (onlyIdx == null) {
    for (let i = 0; i < numItems; i++)
      each(i, coord(i, offs, iwid, gap), _iwid);
  }
  else
    each(onlyIdx, coord(onlyIdx, offs, iwid, gap), _iwid);
}

function pointWithin(px, py, rlft, rtop, rrgt, rbtm) {
  return px >= rlft && px <= rrgt && py >= rtop && py <= rbtm;
}

var Quadtree = (function() {
  const MAX_OBJECTS = 10;
  const MAX_LEVELS  = 4;

  function Quadtree(x, y, w, h, l) {
    let t = this;
    t.x = x;
    t.y = y;
    t.w = w;
    t.h = h;
    t.l = l || 0;
    t.o = [];
    t.q = null;
  }

  Object.assign(Quadtree.prototype, {
    split: function() {
      let t = this, x = t.x, y = t.y, w = t.w / 2, h = t.h / 2, l = t.l + 1;
      t.q = [
        new Quadtree(x + w, y, w, h, l),
        new Quadtree(x, y, w, h, l),
        new Quadtree(x, y + h, w, h, l),
        new Quadtree(x + w, y + h, w, h, l),
      ];
    },
    quads: function(x, y, w, h, cb) {
      let t = this, q = t.q,
        hzMid = t.x + t.w / 2,
        vtMid = t.y + t.h / 2,
        startIsNorth = y < vtMid,
        startIsWest  = x < hzMid,
        endIsEast    = x + w > hzMid,
        endIsSouth   = y + h > vtMid;
      startIsNorth && endIsEast && cb(q[0]);
      startIsWest && startIsNorth && cb(q[1]);
      startIsWest && endIsSouth && cb(q[2]);
      endIsEast && endIsSouth && cb(q[3]);
    },
    add: function(o) {
      let t = this;
      if (t.q != null) {
        t.quads(o.x, o.y, o.w, o.h, q => { q.add(o); });
      } else {
        let os = t.o;
        os.push(o);
        if (os.length > MAX_OBJECTS && t.l < MAX_LEVELS) {
          t.split();
          for (let i = 0; i < os.length; i++) {
            let oi = os[i];
            t.quads(oi.x, oi.y, oi.w, oi.h, q => { q.add(oi); });
          }
          t.o.length = 0;
        }
      }
    },
    get: function(x, y, w, h, cb) {
      let t = this;
      for (let i = 0; i < t.o.length; i++) cb(t.o[i]);
      if (t.q != null) {
        t.quads(x, y, w, h, q => { q.get(x, y, w, h, cb); });
      }
    },
    clear: function() {
      this.o.length = 0;
      this.q = null;
    },
  });

  return Quadtree;
})();

function seriesBarsPlugin(opts) {
  let pxRatio;

  let { ignore = [] } = opts;
  let radius = opts.radius ?? 0;

  function setPxRatio() { pxRatio = devicePixelRatio; }
  setPxRatio();
  window.addEventListener('dppxchange', setPxRatio);

  const ori        = opts.ori ?? 0;
  const dir        = opts.dir ?? 1;
  const stacked    = opts.stacked;
  const groupWidth = opts.groupWidth ?? 0.9;
  const groupDistr = SPACE_BETWEEN;
  const barWidth   = 1;
  const barDistr   = SPACE_BETWEEN;

  function distrTwo(groupCount, barCount, barSpread = true, _groupWidth = groupWidth) {
    let out = Array.from({length: barCount}, () => ({
      offs: Array(groupCount).fill(0),
      size: Array(groupCount).fill(0),
    }));

    distr(groupCount, _groupWidth, groupDistr, null, (groupIdx, groupOffPct, groupDimPct) => {
      distr(barCount, barWidth, barDistr, null, (barIdx, barOffPct, barDimPct) => {
        out[barIdx].offs[groupIdx] = groupOffPct + (barSpread ? (groupDimPct * barOffPct) : 0);
        out[barIdx].size[groupIdx] = groupDimPct * (barSpread ? barDimPct : 1);
      });
    });

    return out;
  }

  let barsPctLayout;

  let barsBuilder = uPlot.paths.bars({
    radius,
    disp: {
      x0: {
        unit: 2,
        values: (u, seriesIdx) => barsPctLayout[seriesIdx].offs,
      },
      size: {
        unit: 2,
        values: (u, seriesIdx) => barsPctLayout[seriesIdx].size,
      },
      ...opts.disp,
    },
    each: (u, seriesIdx, dataIdx, lft, top, wid, hgt) => {
      lft -= u.bbox.left;
      top -= u.bbox.top;
      qt.add({x: lft, y: top, w: wid, h: hgt, sidx: seriesIdx, didx: dataIdx});
    },
  });

  function range(u, dataMin, dataMax) {
    let [min, max] = uPlot.rangeNum(0, dataMax, 0.05, true);
    return [0, max];
  }

  let qt;
  let hoverRect = null;

  return {
    hooks: {
      drawClear: u => {
        qt = qt || new Quadtree(0, 0, u.bbox.width, u.bbox.height);
        qt.clear();
        u.series.forEach(s => { s._paths = null; });
        barsPctLayout = [null].concat(distrTwo(u.data[0].length, u.series.length - 1 - ignore.length, !stacked, groupWidth));
      },
      draw: u => {
        if (!hoverRect) return;
        let ctx = u.ctx;
        let x = u.bbox.left + hoverRect.x;
        let y = u.bbox.top + hoverRect.y;
        let w = hoverRect.w;
        let h = hoverRect.h;
        let r = Math.min(radius * w, w / 2, h / 2);

        ctx.save();
        ctx.fillStyle = "rgba(255,255,255,0.2)";
        ctx.beginPath();
        ctx.moveTo(x + r, y);
        ctx.arcTo(x + w, y, x + w, y + h, r);
        ctx.arcTo(x + w, y + h, x, y + h, 0);
        ctx.arcTo(x, y + h, x, y, 0);
        ctx.arcTo(x, y, x + w, y, r);
        ctx.closePath();
        ctx.fill();
        ctx.restore();
      },
    },
    opts: (u, opts) => {
      const yScaleOpts = {
        range,
        ori: ori == 0 ? 1 : 0,
      };

      let hRect;

      uPlot.assign(opts, {
        select: {show: false},
        cursor: {
          x: false,
          y: false,
          dataIdx: (u, seriesIdx) => {
            if (seriesIdx == 1) {
              hRect = null;
              let cx = u.cursor.left * pxRatio;
              let cy = u.cursor.top * pxRatio;
              qt.get(cx, cy, 1, 1, o => {
                if (pointWithin(cx, cy, o.x, o.y, o.x + o.w, o.y + o.h))
                  hRect = o;
              });

              let prev = hoverRect;
              hoverRect = hRect;
              if (hRect !== prev) {
                u.redraw(false, false);
              }
            }
            return hRect && seriesIdx == hRect.sidx ? hRect.didx : null;
          },
          points: { show: false },
        },
        scales: {
          x: {
            time: false,
            distr: 2,
            ori,
            dir,
            range: (u, min, max) => {
              min = 0;
              max = Math.max(1, u.data[0].length - 1);
              let pctOffset = 0;
              distr(u.data[0].length, groupWidth, groupDistr, 0, (di, lftPct, widPct) => {
                pctOffset = lftPct + widPct / 2;
              });
              let rn = max - min;
              if (pctOffset == 0.5)
                min -= rn;
              else {
                let upScale = 1 / (1 - pctOffset * 2);
                let offset = (upScale * rn - rn) / 2;
                min -= offset;
                max += offset;
              }
              return [min, max];
            }
          },
          y: yScaleOpts,
        }
      });

      uPlot.assign(opts.axes[0], {
        splits: (u) => {
          const _dir = dir * (ori == 0 ? 1 : -1);
          let splits = u._data[0].slice();
          return _dir == 1 ? splits : splits.reverse();
        },
        values: u => u.data[0],
        gap: 15,
        size: ori == 0 ? 40 : 150,
        labelSize: 20,
        grid: {show: false},
        ticks: {show: false},
        side: ori == 0 ? 2 : 3,
      });

      opts.series.forEach((s, i) => {
        if (i > 0 && !ignore.includes(i)) {
          uPlot.assign(s, {
            paths: barsBuilder,
            points: { show: false },
          });
        }
      });
    }
  };
}
