// Zustand store for Hyper-MEV P2P Pool state management
import { create } from 'zustand';
import { getNodeId } from '../types/global';

// Types
interface NodeStatus {
  node_id: string;
  active_strategy: string | null;
  peer_count: number;
  opportunity_count: number;
  intent_count: number;
  available_capital: Record<string, string>;
  roles: {
    finder_enabled: boolean;
    capital_provider_enabled: boolean;
    executor_enabled: boolean;
  };
  config: {
    finder_fee_bps: number;
    executor_fee_bps: number;
    min_profit_threshold_usd: string;
    max_gas_price_gwei: string;
  };
}

interface Opportunity {
  opp_id: string;
  strategy_id: string;
  finder_node: string;
  received_at: string;
  opportunity: any;
}

interface ExecutionReceipt {
  opp_id: string;
  executor_node: string;
  our_proceeds: string;
  verified_at: string;
  receipt: any;
}

// API helper function
async function callMevApi(method: string, params?: any): Promise<string> {
  const response = await fetch(`/api`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      [method]: params !== undefined ? params : ""
    }),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  const text = await response.text();
  
  // Handle error responses from backend
  if (text.startsWith('Error:') || text.startsWith('error:')) {
    throw new Error(text);
  }
  
  return text;
}

export interface MevState {
  // Connection state
  nodeId: string | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;

  // MEV data
  nodeStatus: NodeStatus | null;
  opportunities: Opportunity[];
  executionReceipts: ExecutionReceipt[];

  // Actions
  initialize: () => Promise<void>;
  clearError: () => void;
  fetchNodeStatus: () => Promise<void>;
  fetchOpportunities: () => Promise<void>;
  fetchExecutionReceipts: () => Promise<void>;
  updateNodeConfig: (config: Partial<NodeStatus['config']>) => Promise<void>;
  addCapital: (asset: string, amount: string) => Promise<void>;
  connectToPeer: (peerNode: string) => Promise<void>;
  toggleRole: (role: 'finder_enabled' | 'capital_provider_enabled' | 'executor_enabled') => Promise<void>;
}

export const useMevStore = create<MevState>((set, get) => ({
  // Initial state
  nodeId: null,
  isConnected: false,
  isLoading: false,
  error: null,
  nodeStatus: null,
  opportunities: [],
  executionReceipts: [],

  // Actions
  initialize: async () => {
    const nodeId = getNodeId();
    set({
      nodeId,
      isConnected: nodeId !== null,
    });
    
    if (nodeId) {
      try {
        await get().fetchNodeStatus();
      } catch (error) {
        console.error('Failed to initialize:', error);
        set({ 
          error: 'Failed to connect to Hyper-MEV node. Make sure the process is running.',
          isConnected: false 
        });
      }
    }
  },

  clearError: () => set({ error: null }),

  fetchNodeStatus: async () => {
    set({ isLoading: true, error: null });
    
    try {
      const response = await callMevApi('GetNodeStatus');
      const status: NodeStatus = JSON.parse(response);
      
      set({
        nodeId: status.node_id,
        nodeStatus: status,
        isConnected: true,
        isLoading: false
      });

      // Fetch additional data
      await Promise.all([
        get().fetchOpportunities(),
        get().fetchExecutionReceipts()
      ]);

    } catch (error) {
      set({ 
        error: `Failed to fetch node status: ${error}`,
        isLoading: false,
        isConnected: false
      });
    }
  },

  fetchOpportunities: async () => {
    try {
      const response = await callMevApi('GetOpportunities');
      const opportunities: Opportunity[] = JSON.parse(response);
      set({ opportunities });
    } catch (error) {
      console.error('Failed to fetch opportunities:', error);
    }
  },

  fetchExecutionReceipts: async () => {
    try {
      const response = await callMevApi('GetExecutionReceipts');
      const receipts: ExecutionReceipt[] = JSON.parse(response);
      set({ executionReceipts: receipts });
    } catch (error) {
      console.error('Failed to fetch execution receipts:', error);
    }
  },

  updateNodeConfig: async (config: Partial<NodeStatus['config']>) => {
    set({ isLoading: true, error: null });
    
    try {
      await callMevApi('UpdateNodeConfig', config);
      // Refresh status
      await get().fetchNodeStatus();
    } catch (error) {
      set({ 
        error: `Failed to update config: ${error}`,
        isLoading: false
      });
    }
  },

  addCapital: async (asset: string, amount: string) => {
    set({ isLoading: true, error: null });
    
    try {
      await callMevApi('AddCapital', { asset, amount });
      // Refresh status
      await get().fetchNodeStatus();
    } catch (error) {
      set({ 
        error: `Failed to add capital: ${error}`,
        isLoading: false
      });
    }
  },

  connectToPeer: async (peerNode: string) => {
    set({ isLoading: true, error: null });
    
    try {
      await callMevApi('ConnectToPeer', peerNode);
      // Refresh status
      await get().fetchNodeStatus();
    } catch (error) {
      set({ 
        error: `Failed to connect to peer: ${error}`,
        isLoading: false
      });
    }
  },

  toggleRole: async (role: 'finder_enabled' | 'capital_provider_enabled' | 'executor_enabled') => {
    const currentStatus = get().nodeStatus;
    if (!currentStatus) return;

    const newValue = !currentStatus.roles[role];
    
    try {
      await get().updateNodeConfig({ [role]: newValue });
    } catch (error) {
      console.error(`Failed to toggle ${role}:`, error);
    }
  }
}));

// Selector hooks for common use cases
export const useMevNodeId = () => useMevStore((state) => state.nodeId);
export const useMevIsConnected = () => useMevStore((state) => state.isConnected);
export const useMevNodeStatus = () => useMevStore((state) => state.nodeStatus);
export const useMevOpportunities = () => useMevStore((state) => state.opportunities);
export const useMevExecutionReceipts = () => useMevStore((state) => state.executionReceipts);
export const useMevIsLoading = () => useMevStore((state) => state.isLoading);
export const useMevError = () => useMevStore((state) => state.error);