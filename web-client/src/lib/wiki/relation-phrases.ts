/*
Domain: Wiki relation rendering
Owns: Canonical Russian labels for direction-explicit generated Wiki graph edges.
Excludes: Graph topology, localized page titles, relation authoring, and related-page layout.
Zone: Wiki presentation contract; consumes relation types and canonical edge endpoints.
*/

export type WikiRelationPresentation = {
  source: string;
  label: string;
  target: string;
};

// Each label names the authored from → to relation without treating page titles as sentence fragments.
// prettier-ignore
export const russianRelationLabels: Record<string, string> = {
  'answers-with': 'ответ с помощью',
  'automated-by': 'автоматизация посредством',
  clarifies: 'уточнение',
  'connected-rewards': 'связь через вознаграждения',
  'constrains-launch-policy': 'ограничение правил запуска',
  'constrains-probabilistic-use': 'ограничение вероятностного применения',
  'current-documentation-surface': 'представление статуса в текущей документации',
  'current-product-surface': 'представление статуса в текущем клиенте',
  customizes: 'настройка',
  'deepens-topic': 'углубление темы',
  defines: 'определение',
  'defines-current-truth': 'определение актуального состояния',
  'defines-status-vocabulary': 'определение словаря статусов',
  'defines-vocabulary': 'определение словаря',
  'defines-wiki-knowledge-graph': 'определение графа знаний вики',
  'depends-on': 'зависимость от',
  'depends-on-assets': 'зависимость от активов',
  details: 'подробное раскрытие',
  'embeds-runtime-kernel': 'встраивание ядра среды исполнения',
  'enables-assets': 'обеспечение работы активов',
  'enables-foreign-assets': 'обеспечение работы внешних активов',
  entrypoint: 'переход к',
  expands: 'расширение',
  'expands-onboarding': 'расширение вводного материала',
  'explained-by': 'объяснение посредством',
  explains: 'объяснение',
  'explains-architecture': 'объяснение архитектуры',
  'explains-assets': 'объяснение активов',
  'explains-automation': 'объяснение автоматизации',
  'explains-governance': 'объяснение управления',
  'explains-instance': 'объяснение экземпляра',
  'explains-launch-line': 'объяснение условий запуска',
  'explains-manifesto-link': 'объяснение связи с манифестом',
  'explains-minting': 'объяснение эмиссии',
  'explains-navigation-layer': 'объяснение навигационного слоя',
  'explains-product': 'объяснение продукта',
  'explains-protection-bias': 'объяснение приоритета защиты',
  'explains-routing': 'объяснение маршрутизации',
  'explains-runtime': 'объяснение среды исполнения',
  'explains-runtime-baseline': 'объяснение основы среды исполнения',
  'explains-standard': 'объяснение стандарта',
  extends: 'дополнение',
  'framed-by': 'представление через',
  frames: 'задание рамок для',
  grounds: 'обоснование',
  guides: 'направление к',
  'implemented-by': 'реализация посредством',
  'implemented-in-standard': 'реализация в стандарте',
  'implements-read-model-honesty': 'реализация честного представления данных для чтения',
  'instance-view': 'представление отдельного экземпляра',
  introduces: 'знакомство с',
  maps: 'отображение структуры',
  'maps-actor-domain': 'отображение домена акторов',
  'maps-client-domain': 'отображение клиентского домена',
  'maps-economic-domain': 'отображение экономического домена',
  'maps-framework-domain': 'отображение домена фреймворка',
  'maps-governance-domain': 'отображение домена управления',
  onboarding: 'вводное знакомство с',
  'onboarding-context': 'вводный контекст для',
  'onboarding-entry': 'начало знакомства с',
  orientation: 'ориентир по',
  'overview-parent': 'включение в общий обзор',
  'paired-with': 'работа в паре с',
  'publishes-observations': 'публикация типизированных наблюдений',
  qualifies: 'ограничение области утверждения',
  'recommended-next': 'рекомендуемый следующий шаг',
  recommends: 'рекомендация',
  references: 'ссылка на',
  'related-subsystem': 'связанная подсистема',
  relates: 'связь с',
  'release-status-boundary': 'граница статуса и выпуска',
  'rendered-by-client': 'отображение клиентом',
  requires: 'требование',
  'route-fork': 'путь к созданию производной системы',
  'route-local-run': 'путь к локальному запуску',
  'route-understand': 'путь к изучению системы',
  routes: 'маршрут к',
  starts: 'начало пути к',
  'status-and-release-route': 'путь к сведениям о статусе и выпуске',
  'subsystem-view': 'выделение подсистемы',
  summarizes: 'обобщение',
  supports: 'поддержка',
  'supports-integration': 'поддержка интеграции с',
  'supports-launch-baseline': 'поддержка основы запуска',
  'supports-runtime-flow': 'поддержка потока исполнения',
  'system-context': 'размещение в контексте системы',
  'system-parent': 'включение в систему',
  'triggers-reconsideration': 'запуск повторной проверки',
  uses: 'использование',
  'uses-runtime-automation': 'использование автоматизации среды исполнения',
  visualizes: 'визуализация',
  'walks-through': 'пошаговый разбор',
};

export const unknownRussianRelationLabel = 'неуточнённая связь';

export function formatRussianRelation(
  type: string,
  sourceTitle: string,
  targetTitle: string,
): WikiRelationPresentation {
  return {
    source: sourceTitle,
    label: russianRelationLabels[type] ?? unknownRussianRelationLabel,
    target: targetTitle,
  };
}
