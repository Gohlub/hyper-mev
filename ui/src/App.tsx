// Hyper-MEV P2P Pool - Main App component
import { useEffect, useState } from 'react';
import './App.css';
import { useMevStore } from './store/mev';

function App() {
  // Store state and actions
  const {
    nodeId,
    isConnected,
    isLoading,
    error,
    nodeStatus,
    opportunities,
    executionReceipts,
    initialize,
    clearError,
    fetchNodeStatus,
    updateNodeConfig,
    addCapital,
    connectToPeer,
    toggleRole,
  } = useMevStore();

  // Local UI state
  const [newPeerNode, setNewPeerNode] = useState('');
  const [capitalAsset, setCapitalAsset] = useState('');
  const [capitalAmount, setCapitalAmount] = useState('');
  const [showConfigForm, setShowConfigForm] = useState(false);
  const [configForm, setConfigForm] = useState({
    finder_fee_bps: '',
    executor_fee_bps: '',
    min_profit_threshold_usd: '',
    max_gas_price_gwei: '',
  });

  // Initialize on mount
  useEffect(() => {
    initialize();
  }, [initialize]);

  // Auto-refresh status every 30 seconds if connected
  useEffect(() => {
    if (!isConnected) return;
    
    const interval = setInterval(() => {
      fetchNodeStatus();
    }, 30000);
    
    return () => clearInterval(interval);
  }, [isConnected, fetchNodeStatus]);

  // Update config form when nodeStatus changes
  useEffect(() => {
    if (nodeStatus) {
      setConfigForm({
        finder_fee_bps: nodeStatus.config.finder_fee_bps.toString(),
        executor_fee_bps: nodeStatus.config.executor_fee_bps.toString(),
        min_profit_threshold_usd: nodeStatus.config.min_profit_threshold_usd,
        max_gas_price_gwei: nodeStatus.config.max_gas_price_gwei,
      });
    }
  }, [nodeStatus]);

  const handleAddCapital = async () => {
    if (!capitalAsset.trim() || !capitalAmount.trim()) return;
    await addCapital(capitalAsset, capitalAmount);
    setCapitalAsset('');
    setCapitalAmount('');
  };

  const handleConnectPeer = async () => {
    if (!newPeerNode.trim()) return;
    await connectToPeer(newPeerNode);
    setNewPeerNode('');
  };

  const handleUpdateConfig = async () => {
    const config = {
      finder_fee_bps: parseInt(configForm.finder_fee_bps),
      executor_fee_bps: parseInt(configForm.executor_fee_bps),
      min_profit_threshold_usd: configForm.min_profit_threshold_usd,
      max_gas_price_gwei: configForm.max_gas_price_gwei,
    };
    await updateNodeConfig(config);
    setShowConfigForm(false);
  };

  const formatAmount = (amount: string) => {
    try {
      const num = parseFloat(amount);
      return num.toExponential(2);
    } catch {
      return amount;
    }
  };

  return (
    <div className="app">
      {/* Header */}
      <header className="app-header">
        <h1 className="app-title">⚡ Hyper-MEV P2P Pool</h1>
        <div className="node-info">
          {isConnected ? (
            <>
              Connected as <span className="node-id">{nodeId}</span>
              {nodeStatus && (
                <span className="strategy-info">
                  | Strategy: {nodeStatus.active_strategy || 'None'}
                  | Peers: {nodeStatus.peer_count}
                </span>
              )}
            </>
          ) : (
            <span className="not-connected">Not connected to Hyperware</span>
          )}
        </div>
      </header>

      {/* Error display */}
      {error && (
        <div className="error error-message">
          {error}
          <button onClick={clearError} style={{ marginLeft: '1rem' }}>
            Dismiss
          </button>
        </div>
      )}

      {/* Main content */}
      {isConnected && nodeStatus && (
        <>
          {/* Node Status Section */}
          <section className="section">
            <h2 className="section-title">Node Status</h2>
            
            <div className="status-grid">
              <div className="status-card">
                <h3>Roles</h3>
                <div className="role-toggles">
                  <label>
                    <input
                      type="checkbox"
                      checked={nodeStatus.roles.finder_enabled}
                      onChange={() => toggleRole('finder_enabled')}
                    />
                    Finder (Spot opportunities)
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={nodeStatus.roles.capital_provider_enabled}
                      onChange={() => toggleRole('capital_provider_enabled')}
                    />
                    Capital Provider
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={nodeStatus.roles.executor_enabled}
                      onChange={() => toggleRole('executor_enabled')}
                    />
                    Executor (Execute transactions)
                  </label>
                </div>
              </div>

              <div className="status-card">
                <h3>Activity</h3>
                <div className="activity-stats">
                  <div>Active Opportunities: {nodeStatus.opportunity_count}</div>
                  <div>Submitted Intents: {nodeStatus.intent_count}</div>
                  <div>Connected Peers: {nodeStatus.peer_count}</div>
                </div>
              </div>

              <div className="status-card">
                <h3>Configuration</h3>
                <div className="config-display">
                  <div>Finder Fee: {nodeStatus.config.finder_fee_bps} bps</div>
                  <div>Executor Fee: {nodeStatus.config.executor_fee_bps} bps</div>
                  <div>Min Profit: {formatAmount(nodeStatus.config.min_profit_threshold_usd)} USD</div>
                  <div>Max Gas: {nodeStatus.config.max_gas_price_gwei} gwei</div>
                </div>
                <button 
                  onClick={() => setShowConfigForm(!showConfigForm)}
                  className="config-button"
                >
                  {showConfigForm ? 'Cancel' : 'Edit Config'}
                </button>
              </div>
            </div>

            {/* Config Form */}
            {showConfigForm && (
              <div className="config-form">
                <h3>Update Configuration</h3>
                <div className="form-group">
                  <label>Finder Fee (bps):</label>
                  <input
                    type="number"
                    value={configForm.finder_fee_bps}
                    onChange={(e) => setConfigForm({...configForm, finder_fee_bps: e.target.value})}
                  />
                </div>
                <div className="form-group">
                  <label>Executor Fee (bps):</label>
                  <input
                    type="number"
                    value={configForm.executor_fee_bps}
                    onChange={(e) => setConfigForm({...configForm, executor_fee_bps: e.target.value})}
                  />
                </div>
                <div className="form-group">
                  <label>Min Profit Threshold (wei):</label>
                  <input
                    type="text"
                    value={configForm.min_profit_threshold_usd}
                    onChange={(e) => setConfigForm({...configForm, min_profit_threshold_usd: e.target.value})}
                  />
                </div>
                <div className="form-group">
                  <label>Max Gas Price (gwei):</label>
                  <input
                    type="text"
                    value={configForm.max_gas_price_gwei}
                    onChange={(e) => setConfigForm({...configForm, max_gas_price_gwei: e.target.value})}
                  />
                </div>
                <div className="button-group">
                  <button onClick={handleUpdateConfig} disabled={isLoading}>
                    Update Config
                  </button>
                  <button onClick={() => setShowConfigForm(false)}>
                    Cancel
                  </button>
                </div>
              </div>
            )}
          </section>

          {/* Capital Management Section */}
          <section className="section">
            <h2 className="section-title">Capital Management</h2>
            
            <div className="capital-section">
              <div className="available-capital">
                <h3>Available Capital</h3>
                {Object.keys(nodeStatus.available_capital).length > 0 ? (
                  Object.entries(nodeStatus.available_capital).map(([asset, amount]) => (
                    <div key={asset} className="capital-item">
                      <span className="asset">{asset}:</span>
                      <span className="amount">{formatAmount(amount)}</span>
                    </div>
                  ))
                ) : (
                  <div className="no-capital">No capital allocated</div>
                )}
              </div>

              <div className="add-capital">
                <h3>Add Capital</h3>
                <div className="form-group">
                  <input
                    type="text"
                    placeholder="Asset address (0x...)"
                    value={capitalAsset}
                    onChange={(e) => setCapitalAsset(e.target.value)}
                  />
                  <input
                    type="text"
                    placeholder="Amount (wei)"
                    value={capitalAmount}
                    onChange={(e) => setCapitalAmount(e.target.value)}
                  />
                  <button onClick={handleAddCapital} disabled={isLoading}>
                    Add Capital
                  </button>
                </div>
              </div>
            </div>
          </section>

          {/* P2P Network Section */}
          <section className="section">
            <h2 className="section-title">P2P Network</h2>
            
            <div className="peer-management">
              <h3>Connect to Peer</h3>
              <div className="form-group">
                <input
                  type="text"
                  placeholder="Peer node ID (e.g., alice.os)"
                  value={newPeerNode}
                  onChange={(e) => setNewPeerNode(e.target.value)}
                />
                <button onClick={handleConnectPeer} disabled={isLoading}>
                  Connect
                </button>
              </div>
            </div>
          </section>

          {/* Opportunities Section */}
          <section className="section">
            <h2 className="section-title">MEV Opportunities</h2>
            
            <div className="opportunities-list">
              {opportunities.length > 0 ? (
                opportunities.map((opp) => (
                  <div key={opp.opp_id} className="opportunity-item">
                    <div className="opp-header">
                      <span className="opp-id">{opp.opp_id}</span>
                      <span className="strategy">{opp.strategy_id}</span>
                      <span className="finder">Found by: {opp.finder_node}</span>
                    </div>
                    <div className="opp-time">{new Date(opp.received_at).toLocaleString()}</div>
                  </div>
                ))
              ) : (
                <div className="no-opportunities">No active opportunities</div>
              )}
            </div>
          </section>

          {/* Execution Receipts Section */}
          <section className="section">
            <h2 className="section-title">Execution History</h2>
            
            <div className="receipts-list">
              {executionReceipts.length > 0 ? (
                executionReceipts.map((receipt, index) => (
                  <div key={index} className="receipt-item">
                    <div className="receipt-header">
                      <span className="opp-id">{receipt.opp_id}</span>
                      <span className="executor">Executed by: {receipt.executor_node}</span>
                      <span className="proceeds">Our proceeds: {formatAmount(receipt.our_proceeds)}</span>
                    </div>
                    <div className="receipt-time">{new Date(receipt.verified_at).toLocaleString()}</div>
                  </div>
                ))
              ) : (
                <div className="no-receipts">No execution history</div>
              )}
            </div>
          </section>

          {/* Refresh Button */}
          <div className="refresh-section">
            <button onClick={fetchNodeStatus} disabled={isLoading}>
              {isLoading ? 'Loading...' : 'Refresh Status'}
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default App;