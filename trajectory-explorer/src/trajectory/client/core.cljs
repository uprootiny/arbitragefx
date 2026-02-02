(ns trajectory.client.core
  (:require [reagent.core :as r]
            [reagent.dom :as rdom]
            [re-frame.core :as rf]
            [cljs-http.client :as http]
            [cljs.core.async :refer [<!]])
  (:require-macros [cljs.core.async.macros :refer [go]]))

;; =============================================================================
;; State
;; =============================================================================

(rf/reg-event-db
 :initialize
 (fn [_ _]
   {:loading true
    :view :trajectory
    :commits []
    :ledger {:hypotheses {} :evidence []}
    :trajectory nil
    :selected-commit nil
    :api-base ""}))

(rf/reg-event-db
 :set-loading
 (fn [db [_ loading]]
   (assoc db :loading loading)))

(rf/reg-event-db
 :set-commits
 (fn [db [_ commits]]
   (assoc db :commits commits)))

(rf/reg-event-db
 :set-ledger
 (fn [db [_ ledger]]
   (assoc db :ledger ledger)))

(rf/reg-event-db
 :set-trajectory
 (fn [db [_ data]]
   (assoc db :trajectory data :loading false)))

(rf/reg-event-db
 :set-view
 (fn [db [_ view]]
   (assoc db :view view)))

(rf/reg-event-db
 :select-commit
 (fn [db [_ commit]]
   (assoc db :selected-commit commit)))

(rf/reg-event-db
 :set-api-base
 (fn [db [_ base]]
   (assoc db :api-base base)))

;; Subscriptions
(rf/reg-sub :loading (fn [db] (:loading db)))
(rf/reg-sub :view (fn [db] (:view db)))
(rf/reg-sub :commits (fn [db] (:commits db)))
(rf/reg-sub :ledger (fn [db] (:ledger db)))
(rf/reg-sub :trajectory (fn [db] (:trajectory db)))
(rf/reg-sub :selected-commit (fn [db] (:selected-commit db)))
(rf/reg-sub :api-base (fn [db] (:api-base db)))

;; =============================================================================
;; API Calls
;; =============================================================================

(defn fetch-data! [api-base]
  (go
    (let [trajectory-resp (<! (http/get (str api-base "/api/trajectory")))
          ledger-resp (<! (http/get (str api-base "/api/ledger")))
          commits-resp (<! (http/get (str api-base "/api/commits")))]
      (when (:success trajectory-resp)
        (rf/dispatch [:set-trajectory (:body trajectory-resp)]))
      (when (:success ledger-resp)
        (rf/dispatch [:set-ledger (:body ledger-resp)]))
      (when (:success commits-resp)
        (rf/dispatch [:set-commits (:commits (:body commits-resp))])))))

;; =============================================================================
;; Components
;; =============================================================================

(defn nav-bar []
  (let [view @(rf/subscribe [:view])]
    [:nav {:class "nav-bar"}
     [:h1 "Trajectory Explorer"]
     [:div {:class "nav-links"}
      [:button {:class (when (= view :trajectory) "active")
                :on-click #(rf/dispatch [:set-view :trajectory])}
       "Trajectory"]
      [:button {:class (when (= view :commits) "active")
                :on-click #(rf/dispatch [:set-view :commits])}
       "Commits"]
      [:button {:class (when (= view :hypotheses) "active")
                :on-click #(rf/dispatch [:set-view :hypotheses])}
       "Hypotheses"]
      [:button {:class (when (= view :futures) "active")
                :on-click #(rf/dispatch [:set-view :futures])}
       "Futures"]]]))

(defn category-badge [category]
  (let [colors {:research "#4CAF50"
                :strategy "#2196F3"
                :testing "#FF9800"
                :bugfix "#f44336"
                :refactor "#9C27B0"
                :documentation "#607D8B"
                :feature "#00BCD4"
                :other "#757575"}]
    [:span {:class "badge"
            :style {:background-color (get colors category "#757575")}}
     (name category)]))

(defn commit-item [{:keys [hash timestamp author subject category]}]
  (let [selected @(rf/subscribe [:selected-commit])
        is-selected (= hash (:hash selected))]
    [:div {:class (str "commit-item" (when is-selected " selected"))
           :on-click #(rf/dispatch [:select-commit {:hash hash :subject subject}])}
     [:div {:class "commit-header"}
      [:code {:class "hash"} (subs hash 0 7)]
      [category-badge category]
      [:span {:class "date"}
       (-> timestamp (* 1000) js/Date. .toLocaleDateString)]]
     [:div {:class "commit-subject"} subject]
     [:div {:class "commit-author"} author]]))

(defn commits-view []
  (let [commits @(rf/subscribe [:commits])
        trajectory @(rf/subscribe [:trajectory])
        classified (get-in trajectory [:trajectory :commits] commits)]
    [:div {:class "commits-view"}
     [:h2 "Commit History"]
     [:div {:class "commit-list"}
      (for [commit classified]
        ^{:key (:hash commit)}
        [commit-item commit])]]))

(defn hypothesis-card [{:keys [id statement status] :as h}]
  (let [status-colors {"Supported" "#4CAF50"
                       "Refuted" "#f44336"
                       "Testing" "#FF9800"
                       "Proposed" "#2196F3"
                       "Inconclusive" "#9C27B0"}]
    [:div {:class "hypothesis-card"}
     [:div {:class "hyp-header"}
      [:span {:class "hyp-id"} id]
      [:span {:class "status-badge"
              :style {:background-color (get status-colors status "#757575")}}
       status]]
     [:p {:class "hyp-statement"} statement]]))

(defn hypotheses-view []
  (let [ledger @(rf/subscribe [:ledger])
        hypotheses (vals (:hypotheses ledger))
        evidence (:evidence ledger)]
    [:div {:class "hypotheses-view"}
     [:h2 "Hypothesis Ledger"]
     [:div {:class "stats"}
      [:span (str (count hypotheses) " hypotheses")]
      [:span (str (count evidence) " evidence points")]]
     [:div {:class "hypothesis-grid"}
      (for [h hypotheses]
        ^{:key (:id h)}
        [hypothesis-card h])]
     (when (seq evidence)
       [:div {:class "evidence-section"}
        [:h3 "Recent Evidence"]
        [:table
         [:thead
          [:tr [:th "Hypothesis"] [:th "Regime"] [:th "Sharpe"] [:th "Result"]]]
         [:tbody
          (for [e (take 10 evidence)]
            ^{:key (:id e)}
            [:tr
             [:td (:hypothesis_id e)]
             [:td (:regime e)]
             [:td (.toFixed (or (get-in e [:metrics :sharpe]) 0) 2)]
             [:td (if (:supports_hypothesis e) "Supports" "Refutes")]])]]])]))

(defn trajectory-view []
  (let [trajectory @(rf/subscribe [:trajectory])
        by-category (get-in trajectory [:trajectory :by-category] {})
        focus (get-in trajectory [:trajectory :recent-focus])
        suggestions (get trajectory :suggestions [])]
    [:div {:class "trajectory-view"}
     [:h2 "Development Trajectory"]

     [:div {:class "trajectory-stats"}
      [:h3 "Activity by Category"]
      [:div {:class "category-bars"}
       (for [[cat count] (sort-by val > by-category)]
         ^{:key cat}
         [:div {:class "category-bar"}
          [:span {:class "cat-label"} (name cat)]
          [:div {:class "bar-container"}
           [:div {:class "bar"
                  :style {:width (str (* 10 count) "px")}}]]
          [:span {:class "count"} count]])]]

     (when focus
       [:div {:class "focus-indicator"}
        [:h3 "Recent Focus"]
        [category-badge focus]])

     (when (seq suggestions)
       [:div {:class "suggestions"}
        [:h3 "Suggested Next Steps"]
        [:ul
         (for [[idx s] (map-indexed vector suggestions)]
           ^{:key idx}
           [:li
            [:span {:class (str "priority " (name (:priority s)))}
             (name (:priority s))]
            [:span {:class "suggestion-type"} (name (:type s))]
            [:p (:description s)]])]])]))

(defn futures-view []
  (let [trajectory @(rf/subscribe [:trajectory])
        futures (get trajectory :futures [])]
    [:div {:class "futures-view"}
     [:h2 "Possible Futures"]
     [:p {:class "subtitle"} "Extrapolated development trajectories"]

     [:div {:class "futures-grid"}
      (for [f futures]
        ^{:key (:id f)}
        [:div {:class "future-card"}
         [:h3 (:name f)]
         [:div {:class "probability"}
          (str "Probability: " (int (* 100 (:probability f))) "%")]
         [:div {:class "probability-bar"}
          [:div {:class "fill"
                 :style {:width (str (* 100 (:probability f)) "%")}}]]
         [:ol {:class "steps"}
          (for [[idx step] (map-indexed vector (:steps f))]
            ^{:key idx}
            [:li step])]])]]))

(defn loading-spinner []
  [:div {:class "loading"}
   [:div {:class "spinner"}]
   [:p "Loading trajectory data..."]])

(defn app []
  (let [loading @(rf/subscribe [:loading])
        view @(rf/subscribe [:view])]
    [:div {:class "app"}
     [nav-bar]
     [:main
      (if loading
        [loading-spinner]
        (case view
          :trajectory [trajectory-view]
          :commits [commits-view]
          :hypotheses [hypotheses-view]
          :futures [futures-view]
          [trajectory-view]))]]))

;; =============================================================================
;; Styles (inline for simplicity)
;; =============================================================================

(def styles "
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
         background: #1a1a2e; color: #eee; }
  .app { min-height: 100vh; }

  .nav-bar { display: flex; justify-content: space-between; align-items: center;
             padding: 1rem 2rem; background: #16213e; border-bottom: 1px solid #0f3460; }
  .nav-bar h1 { font-size: 1.5rem; color: #e94560; }
  .nav-links { display: flex; gap: 0.5rem; }
  .nav-links button { padding: 0.5rem 1rem; background: #0f3460; color: #eee;
                      border: none; border-radius: 4px; cursor: pointer; }
  .nav-links button:hover { background: #1a4085; }
  .nav-links button.active { background: #e94560; }

  main { padding: 2rem; max-width: 1400px; margin: 0 auto; }

  h2 { color: #e94560; margin-bottom: 1rem; }
  h3 { color: #eee; margin: 1rem 0 0.5rem; }

  .loading { display: flex; flex-direction: column; align-items: center;
             justify-content: center; height: 50vh; }
  .spinner { width: 50px; height: 50px; border: 4px solid #0f3460;
             border-top-color: #e94560; border-radius: 50%;
             animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .badge { padding: 0.2rem 0.5rem; border-radius: 3px; font-size: 0.75rem;
           color: white; text-transform: uppercase; }

  .commit-list { display: flex; flex-direction: column; gap: 0.5rem; }
  .commit-item { padding: 1rem; background: #16213e; border-radius: 8px;
                 cursor: pointer; border: 1px solid transparent; }
  .commit-item:hover { border-color: #0f3460; }
  .commit-item.selected { border-color: #e94560; }
  .commit-header { display: flex; gap: 1rem; align-items: center; margin-bottom: 0.5rem; }
  .hash { color: #e94560; }
  .date { color: #888; font-size: 0.85rem; }
  .commit-subject { font-weight: 500; margin-bottom: 0.25rem; }
  .commit-author { color: #888; font-size: 0.85rem; }

  .hypothesis-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                     gap: 1rem; }
  .hypothesis-card { padding: 1rem; background: #16213e; border-radius: 8px; }
  .hyp-header { display: flex; justify-content: space-between; margin-bottom: 0.5rem; }
  .hyp-id { font-family: monospace; color: #e94560; }
  .status-badge { padding: 0.2rem 0.5rem; border-radius: 3px; font-size: 0.75rem; color: white; }
  .hyp-statement { color: #ccc; font-size: 0.9rem; }

  .stats { display: flex; gap: 2rem; margin-bottom: 1rem; color: #888; }

  .evidence-section { margin-top: 2rem; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.75rem; text-align: left; border-bottom: 1px solid #0f3460; }
  th { background: #16213e; color: #e94560; }

  .trajectory-stats { margin-bottom: 2rem; }
  .category-bars { display: flex; flex-direction: column; gap: 0.5rem; }
  .category-bar { display: flex; align-items: center; gap: 1rem; }
  .cat-label { width: 100px; text-transform: capitalize; }
  .bar-container { flex: 1; background: #0f3460; height: 20px; border-radius: 3px; }
  .bar { background: #e94560; height: 100%; border-radius: 3px; transition: width 0.3s; }
  .count { width: 40px; text-align: right; color: #888; }

  .focus-indicator { padding: 1rem; background: #16213e; border-radius: 8px;
                     margin-bottom: 1rem; display: inline-block; }

  .suggestions { margin-top: 1rem; }
  .suggestions ul { list-style: none; }
  .suggestions li { padding: 1rem; background: #16213e; border-radius: 8px;
                    margin-bottom: 0.5rem; display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .priority { padding: 0.2rem 0.5rem; border-radius: 3px; font-size: 0.75rem;
              text-transform: uppercase; }
  .priority.high { background: #f44336; }
  .priority.medium { background: #FF9800; }
  .priority.low { background: #4CAF50; }
  .suggestion-type { background: #0f3460; padding: 0.2rem 0.5rem; border-radius: 3px; }
  .suggestions p { width: 100%; margin-top: 0.5rem; color: #ccc; }

  .futures-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
                  gap: 1rem; }
  .future-card { padding: 1.5rem; background: #16213e; border-radius: 8px; }
  .future-card h3 { color: #e94560; margin-bottom: 0.5rem; }
  .probability { color: #888; font-size: 0.9rem; }
  .probability-bar { height: 8px; background: #0f3460; border-radius: 4px;
                     margin: 0.5rem 0 1rem; overflow: hidden; }
  .probability-bar .fill { height: 100%; background: #e94560; border-radius: 4px; }
  .steps { padding-left: 1.5rem; color: #ccc; }
  .steps li { margin: 0.5rem 0; }

  .subtitle { color: #888; margin-bottom: 1.5rem; }
")

;; =============================================================================
;; Init
;; =============================================================================

(defn ^:export init []
  ;; Inject styles
  (let [style-el (.createElement js/document "style")]
    (set! (.-textContent style-el) styles)
    (.appendChild (.-head js/document) style-el))

  ;; Initialize re-frame
  (rf/dispatch-sync [:initialize])

  ;; Detect API base (same origin or configured)
  (let [api-base (or (.-TRAJECTORY_API_BASE js/window) "")]
    (rf/dispatch [:set-api-base api-base])
    (fetch-data! api-base))

  ;; Render
  (rdom/render [app] (.getElementById js/document "app")))
