(ns epistemic.core
  "Multi-view epistemic dashboard for tracing data through verification strata.

   Epistemic levels (color-coded):
   - ASSERTED:    Just stated, no verification yet (orange)
   - VERIFIED:    Confirmed by test/backtest (green)
   - USED:        Actually used in decisions (blue)
   - INVARIANT:   Proven to hold always (purple)
   - ASSUMED:     Taken as given, not tested (yellow)
   - INFERRED:    Derived from other data (cyan)
   - EXTRAPOLATED: Projected beyond known (pink)
   - ASPIRATIONAL: Hoped for, not yet real (gray)"
  (:require [reagent.core :as r]
            [reagent.dom :as rdom]
            [cljs.core.async :refer [<! timeout]]
            [clojure.string :as str])
  (:require-macros [cljs.core.async.macros :refer [go-loop]]))

;; =============================================================================
;; Epistemic Levels
;; =============================================================================

(def epistemic-levels
  {:asserted    {:label "Asserted"    :color "#FF9800" :order 0}
   :verified    {:label "Verified"    :color "#4CAF50" :order 1}
   :used        {:label "Used"        :color "#2196F3" :order 2}
   :invariant   {:label "Invariant"   :color "#9C27B0" :order 3}
   :assumed     {:label "Assumed"     :color "#FFEB3B" :order 4}
   :inferred    {:label "Inferred"    :color "#00BCD4" :order 5}
   :extrapolated {:label "Extrapolated" :color "#E91E63" :order 6}
   :aspirational {:label "Aspirational" :color "#9E9E9E" :order 7}})

;; =============================================================================
;; Data Store - The Single Source of Truth
;; =============================================================================

(defonce data-store
  (r/atom
   {:hypotheses
    [{:id "H001" :name "Momentum continuation"
      :level :verified :confidence 0.62
      :evidence [{:regime :bear :sharpe -0.12 :status :refuted}]
      :used-in [:strategy-lab]}
     {:id "H004" :name "Trend filter improves quality"
      :level :verified :confidence 0.78
      :evidence [{:regime :bear :sharpe 0.62 :status :supported}]
      :used-in [:composite-strategy :live-engine]}
     {:id "H006" :name "Short-selling in bear markets"
      :level :verified :confidence 0.92
      :evidence [{:regime :bear :sharpe 1.96 :status :supported}]
      :used-in [:live-engine]}
     {:id "H008" :name "Volatility breakouts"
      :level :verified :confidence 0.71
      :evidence [{:regime :bear :sharpe 0.45 :status :supported}]
      :used-in [:strategy-lab]}
     {:id "H009" :name "Bull market momentum"
      :level :assumed :confidence 0.5
      :evidence []
      :used-in []}
     {:id "H010" :name "Cross-asset correlation"
      :level :aspirational :confidence 0.3
      :evidence []
      :used-in []}]

    :signals
    [{:id "momentum" :level :verified :source "indicators.rs"
      :flows-to [:strategy :filter]}
     {:id "trend" :level :verified :source "indicators.rs"
      :flows-to [:filter :sizing]}
     {:id "rsi" :level :verified :source "indicators.rs"
      :flows-to [:strategy]}
     {:id "macd" :level :verified :source "indicators.rs"
      :flows-to [:strategy]}
     {:id "funding-rate" :level :assumed :source "aux_data.rs"
      :flows-to [:carry-strategy]}
     {:id "liquidations" :level :inferred :source "aux_data.rs"
      :flows-to [:risk-filter]}]

    :filters
    [{:id "volatility" :level :verified :blocks [:high-vol :low-vol]}
     {:id "trend-align" :level :verified :blocks [:counter-trend]}
     {:id "position-limit" :level :invariant :blocks [:over-exposure]}
     {:id "drawdown" :level :invariant :blocks [:max-dd-breach]}
     {:id "daily-limit" :level :verified :blocks [:overtrading]}]

    :sizing
    [{:id "fixed" :level :verified :used true}
     {:id "kelly" :level :verified :used false :reason "needs more data"}
     {:id "vol-adjusted" :level :verified :used true}
     {:id "risk-based" :level :verified :used true}]

    :strategies
    [{:id "momentum-filtered" :level :verified
      :uses [:momentum :trend :vol-adjusted]
      :performance {:sharpe 0.30 :trades 6}}
     {:id "bear-short" :level :verified
      :uses [:trend :momentum :risk-based]
      :performance {:sharpe 1.96 :trades 166}}
     {:id "composite" :level :asserted
      :uses [:bear-short :momentum-filtered :regime-detect]
      :performance nil}
     {:id "adaptive" :level :aspirational
      :uses [:all-hypotheses :ml-regime]
      :performance nil}]

    :dataflows
    [{:from :market-data :to :indicators :level :verified}
     {:from :indicators :to :signals :level :verified}
     {:from :signals :to :filters :level :verified}
     {:from :filters :to :strategy :level :verified}
     {:from :strategy :to :sizing :level :verified}
     {:from :sizing :to :order :level :verified}
     {:from :order :to :exchange :level :assumed}
     {:from :exchange :to :fill :level :inferred}
     {:from :fill :to :ledger :level :verified}
     {:from :ledger :to :metrics :level :verified}
     {:from :metrics :to :hypothesis :level :verified}
     {:from :hypothesis :to :strategy :level :asserted}]

    :invariants
    [{:id "position-bounded" :holds true :level :invariant
      :statement "abs(position) <= max_position"}
     {:id "equity-positive" :holds true :level :invariant
      :statement "equity > 0"}
     {:id "no-naked-shorts" :holds true :level :invariant
      :statement "short_position <= borrowable"}
     {:id "fee-accounted" :holds true :level :invariant
      :statement "all trades include fee estimation"}]

    :assumptions
    [{:id "market-liquid" :level :assumed
      :statement "Can execute at quoted price within slippage"}
     {:id "exchange-reliable" :level :assumed
      :statement "Exchange API available 99.9% of time"}
     {:id "data-accurate" :level :assumed
      :statement "Historical data reflects actual prices"}
     {:id "regime-detectable" :level :extrapolated
      :statement "Market regime can be identified in real-time"}]}))

;; =============================================================================
;; View Components
;; =============================================================================

(defn level-badge [level]
  (let [{:keys [label color]} (get epistemic-levels level)]
    [:span.badge {:style {:background-color color
                          :color (if (= level :assumed) "#333" "#fff")}}
     label]))

(defn confidence-bar [confidence]
  [:div.confidence-bar
   [:div.fill {:style {:width (str (* 100 confidence) "%")
                       :background (cond
                                     (> confidence 0.8) "#4CAF50"
                                     (> confidence 0.5) "#FF9800"
                                     :else "#f44336")}}]
   [:span.value (str (int (* 100 confidence)) "%")]])

;; View 1: Hypothesis View
(defn hypothesis-view []
  (let [hypotheses (:hypotheses @data-store)]
    [:div.view-panel
     [:h3 "Hypotheses"]
     [:div.items
      (for [h hypotheses]
        ^{:key (:id h)}
        [:div.item {:class (name (:level h))}
         [:div.header
          [:span.id (:id h)]
          [level-badge (:level h)]]
         [:div.name (:name h)]
         [confidence-bar (:confidence h)]
         (when (seq (:evidence h))
           [:div.evidence
            (for [e (:evidence h)]
              ^{:key (str (:id h) "-" (:regime e))}
              [:span.evidence-item
               {:class (name (:status e))}
               (str (name (:regime e)) ": " (.toFixed (:sharpe e) 2))])])])]]))

;; View 2: Signal Flow View
(defn signal-flow-view []
  (let [signals (:signals @data-store)
        flows (:dataflows @data-store)]
    [:div.view-panel
     [:h3 "Signal Flows"]
     [:div.flow-diagram
      (for [f flows]
        ^{:key (str (:from f) "-" (:to f))}
        [:div.flow-item {:class (name (:level f))}
         [:span.from (name (:from f))]
         [:span.arrow "→"]
         [:span.to (name (:to f))]
         [level-badge (:level f)]])]
     [:h4 "Active Signals"]
     [:div.items
      (for [s signals]
        ^{:key (:id s)}
        [:div.item {:class (name (:level s))}
         [:span.id (:id s)]
         [level-badge (:level s)]
         [:span.source (:source s)]])]]))

;; View 3: Filter/Invariant View
(defn filter-invariant-view []
  (let [filters (:filters @data-store)
        invariants (:invariants @data-store)]
    [:div.view-panel
     [:h3 "Filters & Invariants"]
     [:div.section
      [:h4 "Trade Filters"]
      [:div.items
       (for [f filters]
         ^{:key (:id f)}
         [:div.item {:class (name (:level f))}
          [:span.id (:id f)]
          [level-badge (:level f)]
          [:span.blocks (str "blocks: " (str/join ", " (map name (:blocks f))))]])]]
     [:div.section
      [:h4 "Invariants"]
      [:div.items
       (for [i invariants]
         ^{:key (:id i)}
         [:div.item.invariant {:class (if (:holds i) "holds" "violated")}
          [:span.status (if (:holds i) "✓" "✗")]
          [:code (:statement i)]])]]]))

;; View 4: Strategy Composition View
(defn strategy-view []
  (let [strategies (:strategies @data-store)
        sizing (:sizing @data-store)]
    [:div.view-panel
     [:h3 "Strategies & Sizing"]
     [:div.section
      [:h4 "Strategies"]
      [:div.items
       (for [s strategies]
         ^{:key (:id s)}
         [:div.item {:class (name (:level s))}
          [:div.header
           [:span.id (:id s)]
           [level-badge (:level s)]]
          [:div.uses "uses: " (str/join ", " (map name (:uses s)))]
          (when-let [perf (:performance s)]
            [:div.perf
             [:span "Sharpe: " (.toFixed (:sharpe perf) 2)]
             [:span "Trades: " (:trades perf)]])])]]
     [:div.section
      [:h4 "Position Sizing"]
      [:div.items
       (for [sz sizing]
         ^{:key (:id sz)}
         [:div.item {:class (str (name (:level sz)) (when (:used sz) " active"))}
          [:span.id (:id sz)]
          [level-badge (:level sz)]
          [:span.status (if (:used sz) "ACTIVE" (or (:reason sz) "inactive"))]])]]]))

;; View 5: Assumptions View
(defn assumptions-view []
  (let [assumptions (:assumptions @data-store)]
    [:div.view-panel
     [:h3 "Assumptions & Extrapolations"]
     [:div.items
      (for [a assumptions]
        ^{:key (:id a)}
        [:div.item {:class (name (:level a))}
         [:div.header
          [:span.id (:id a)]
          [level-badge (:level a)]]
         [:div.statement (:statement a)]])]]))

;; View 6: Dataflow Trace View
(defn dataflow-trace-view []
  (let [flows (:dataflows @data-store)]
    [:div.view-panel
     [:h3 "Dataflow Trace"]
     [:div.trace
      [:svg {:width "100%" :height 300 :viewBox "0 0 800 300"}
       ;; Nodes
       (let [nodes [:market-data :indicators :signals :filters
                    :strategy :sizing :order :exchange :fill
                    :ledger :metrics :hypothesis]
             node-x (fn [i] (+ 50 (* i 60)))
             node-y (fn [i] (+ 50 (* (mod i 3) 80)))]
         [:g
          (for [[i node] (map-indexed vector nodes)]
            ^{:key node}
            [:g
             [:circle {:cx (node-x i) :cy (node-y i) :r 20
                       :fill (get-in epistemic-levels
                                     [(-> flows
                                          (filter #(= (:to %) node))
                                          first
                                          :level
                                          (or :verified))
                                      :color])}]
             [:text {:x (node-x i) :y (+ (node-y i) 35)
                     :text-anchor "middle" :font-size 10}
              (name node)]])])]]]))

;; =============================================================================
;; Layout Components
;; =============================================================================

(defn view-grid [n]
  (let [views [hypothesis-view signal-flow-view filter-invariant-view
               strategy-view assumptions-view dataflow-trace-view
               hypothesis-view signal-flow-view]]
    [:div.view-grid {:class (str "grid-" n)}
     (for [i (range n)]
       ^{:key i}
       [(nth views (mod i (count views)))])]))

(defn legend []
  [:div.legend
   [:h4 "Epistemic Levels"]
   [:div.items
    (for [[k {:keys [label color]}] (sort-by (comp :order val) epistemic-levels)]
      ^{:key k}
      [:div.legend-item
       [:span.swatch {:style {:background-color color}}]
       [:span.label label]])]])

(defn controls []
  (let [n (r/atom 4)]
    (fn []
      [:div.controls
       [:span "Views: "]
       (for [count [2 4 6 8]]
         ^{:key count}
         [:button {:class (when (= @n count) "active")
                   :on-click #(reset! n count)}
          count])
       [view-grid @n]])))

;; =============================================================================
;; Main App
;; =============================================================================

(defn app []
  [:div.app
   [:header
    [:h1 "Epistemic Dashboard"]
    [:p.subtitle "Tracing data through verification strata"]]
   [legend]
   [controls]])

;; =============================================================================
;; Styles
;; =============================================================================

(def styles "
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: 'IBM Plex Mono', monospace; background: #0a0a0f; color: #e0e0e0; }
.app { padding: 1rem; max-width: 1800px; margin: 0 auto; }

header { margin-bottom: 1rem; border-bottom: 1px solid #333; padding-bottom: 1rem; }
header h1 { font-size: 1.5rem; color: #00BCD4; }
.subtitle { color: #666; font-size: 0.85rem; }

.legend { display: flex; gap: 1rem; flex-wrap: wrap; margin-bottom: 1rem;
          padding: 0.5rem; background: #111; border-radius: 4px; }
.legend h4 { width: 100%; font-size: 0.8rem; color: #666; }
.legend-item { display: flex; align-items: center; gap: 0.25rem; }
.swatch { width: 12px; height: 12px; border-radius: 2px; }
.label { font-size: 0.75rem; }

.controls { margin-bottom: 1rem; }
.controls button { padding: 0.25rem 0.75rem; margin-right: 0.5rem;
                   background: #222; color: #888; border: 1px solid #333;
                   border-radius: 3px; cursor: pointer; }
.controls button.active { background: #00BCD4; color: #000; border-color: #00BCD4; }

.view-grid { display: grid; gap: 1rem; }
.grid-2 { grid-template-columns: repeat(2, 1fr); }
.grid-4 { grid-template-columns: repeat(2, 1fr); }
.grid-6 { grid-template-columns: repeat(3, 1fr); }
.grid-8 { grid-template-columns: repeat(4, 1fr); }

.view-panel { background: #111; border: 1px solid #222; border-radius: 4px; padding: 1rem;
              max-height: 400px; overflow-y: auto; }
.view-panel h3 { font-size: 0.9rem; color: #00BCD4; margin-bottom: 0.75rem;
                 border-bottom: 1px solid #222; padding-bottom: 0.5rem; }
.view-panel h4 { font-size: 0.8rem; color: #666; margin: 0.75rem 0 0.5rem; }

.items { display: flex; flex-direction: column; gap: 0.5rem; }
.item { padding: 0.5rem; background: #1a1a1f; border-radius: 3px;
        border-left: 3px solid #333; }
.item.verified { border-left-color: #4CAF50; }
.item.asserted { border-left-color: #FF9800; }
.item.assumed { border-left-color: #FFEB3B; }
.item.inferred { border-left-color: #00BCD4; }
.item.extrapolated { border-left-color: #E91E63; }
.item.aspirational { border-left-color: #9E9E9E; }
.item.invariant { border-left-color: #9C27B0; }
.item.active { background: #1a2a1f; }

.header { display: flex; justify-content: space-between; align-items: center; }
.id { font-weight: bold; color: #00BCD4; }
.name { font-size: 0.85rem; margin: 0.25rem 0; }
.uses, .source, .blocks { font-size: 0.75rem; color: #666; }
.perf { font-size: 0.75rem; color: #4CAF50; display: flex; gap: 1rem; }
.statement { font-size: 0.8rem; font-style: italic; color: #888; }

.badge { padding: 0.15rem 0.4rem; border-radius: 2px; font-size: 0.65rem;
         text-transform: uppercase; font-weight: bold; }

.confidence-bar { height: 4px; background: #333; border-radius: 2px;
                  margin: 0.25rem 0; position: relative; }
.confidence-bar .fill { height: 100%; border-radius: 2px; }
.confidence-bar .value { position: absolute; right: 0; top: -12px;
                         font-size: 0.65rem; color: #666; }

.evidence { display: flex; gap: 0.5rem; margin-top: 0.25rem; }
.evidence-item { font-size: 0.7rem; padding: 0.1rem 0.3rem;
                 background: #222; border-radius: 2px; }
.evidence-item.supported { color: #4CAF50; }
.evidence-item.refuted { color: #f44336; }

.flow-diagram { display: flex; flex-direction: column; gap: 0.25rem; }
.flow-item { display: flex; align-items: center; gap: 0.5rem;
             font-size: 0.8rem; padding: 0.25rem; }
.flow-item .arrow { color: #444; }
.flow-item .from, .flow-item .to { font-family: monospace; }

.invariant.holds { background: #1a2a1f; }
.invariant.violated { background: #2a1a1f; }
.invariant .status { margin-right: 0.5rem; }
.invariant.holds .status { color: #4CAF50; }
.invariant.violated .status { color: #f44336; }
.invariant code { font-size: 0.8rem; }

.trace svg { background: #0a0a0f; }
.trace text { fill: #888; }
")

;; =============================================================================
;; Init
;; =============================================================================

(defn ^:export init []
  (let [style-el (.createElement js/document "style")]
    (set! (.-textContent style-el) styles)
    (.appendChild (.-head js/document) style-el))
  (rdom/render [app] (.getElementById js/document "app")))
