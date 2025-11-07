// Memory Layer - 3D Knowledge Graph Visualization
// Using Three.js and 3d-force-graph for stunning interactive visualization

let graph = null;
let graphData = { nodes: [], links: [] };
let isPaused = false;

// Initialize the 3D force graph when the page loads
document.addEventListener('DOMContentLoaded', () => {
    initializeGraph();
    setupControls();
});

function initializeGraph() {
    const container = document.getElementById('graph');

    // Create the 3D force graph
    graph = ForceGraph3D({
        controlType: 'orbit',
        rendererConfig: {
            antialias: true,
            alpha: true,
            precision: 'highp'
        }
    })(container)
        .backgroundColor('rgba(0,0,0,0)')
        .showNavInfo(false)
        .enableNodeDrag(true)
        .enableNavigationControls(true)
        .enablePointerInteraction(true);

    // Node styling
    graph
        .nodeLabel(node => {
            const keywords = node.keywords && node.keywords.length > 0
                ? node.keywords.slice(0, 3).join(', ')
                : 'No keywords';
            const tags = node.tags && node.tags.length > 0
                ? node.tags.join(', ')
                : 'No tags';
            return `
                <div style="color: white; background: rgba(20, 25, 45, 0.95); padding: 12px; border-radius: 8px; max-width: 300px; backdrop-filter: blur(10px);">
                    <h4 style="margin: 0 0 8px 0; color: #3D85F5; font-weight: 600;">${escapeHtml(node.content.substring(0, 50))}${node.content.length > 50 ? '...' : ''}</h4>
                    <p style="margin: 4px 0; font-size: 12px;"><strong>Context:</strong> ${escapeHtml(node.context.substring(0, 100))}${node.context.length > 100 ? '...' : ''}</p>
                    <p style="margin: 4px 0; font-size: 12px;"><strong>Keywords:</strong> ${escapeHtml(keywords)}</p>
                    <p style="margin: 4px 0; font-size: 12px;"><strong>Tags:</strong> ${escapeHtml(tags)}</p>
                    ${node.category ? `<p style="margin: 4px 0; font-size: 12px;"><strong>Category:</strong> ${escapeHtml(node.category)}</p>` : ''}
                    <p style="margin: 4px 0; font-size: 11px; color: #3D85F5; font-weight: 500;">Retrievals: ${node.retrievalCount || 0}</p>
                </div>
            `;
        })
        .nodeThreeObject(node => {
            // Create glowing sphere for each node
            const geometry = new THREE.SphereGeometry(getNodeSize(node), 32, 32);

            // Node color based on retrieval count (hotness)
            const color = getNodeColor(node);

            const material = new THREE.MeshPhongMaterial({
                color: color,
                emissive: color,
                emissiveIntensity: 0.6,
                shininess: 100,
                transparent: true,
                opacity: 0.9
            });

            const sphere = new THREE.Mesh(geometry, material);

            // Add glow effect for highly accessed nodes
            if (node.retrievalCount > 5) {
                const glowGeometry = new THREE.SphereGeometry(getNodeSize(node) * 1.3, 16, 16);
                const glowMaterial = new THREE.MeshBasicMaterial({
                    color: color,
                    transparent: true,
                    opacity: 0.2,
                    side: THREE.BackSide
                });
                const glow = new THREE.Mesh(glowGeometry, glowMaterial);
                sphere.add(glow);
            }

            return sphere;
        })
        .nodeThreeObjectExtend(true);

    // Link styling
    graph
        .linkLabel(link => {
            return `
                <div style="color: white; background: rgba(20, 25, 45, 0.95); padding: 8px; border-radius: 6px;">
                    <p style="margin: 0; font-size: 12px;"><strong>Strength:</strong> ${(link.strength * 100).toFixed(1)}%</p>
                    ${link.rationale ? `<p style="margin: 4px 0 0 0; font-size: 11px;">${escapeHtml(link.rationale.substring(0, 150))}${link.rationale.length > 150 ? '...' : ''}</p>` : ''}
                </div>
            `;
        })
        .linkColor(link => {
            // Link color based on strength using design system colors
            const strength = link.strength;
            if (strength > 0.7) return '#3D85F5';  // Strong: primary blue
            if (strength > 0.4) return '#5599FF';  // Medium: light blue
            return '#00B894';  // Weak: success green
        })
        .linkWidth(link => link.strength * 2)
        .linkOpacity(0.4)
        .linkDirectionalParticles(link => {
            // Add animated particles for strong connections
            return link.strength > 0.6 ? 2 : 0;
        })
        .linkDirectionalParticleWidth(1.5)
        .linkDirectionalParticleSpeed(0.006);

    // Physics configuration for nice spreading
    graph
        .d3Force('charge', d3.forceManyBody().strength(-300))
        .d3Force('link', d3.forceLink().distance(100).strength(link => link.strength))
        .d3Force('center', d3.forceCenter(0, 0, 0))
        .d3Force('collide', d3.forceCollide(20));

    // Camera configuration
    graph.camera().position.set(0, 0, 400);

    // Add lighting
    const scene = graph.scene();

    // Ambient light for overall illumination
    const ambientLight = new THREE.AmbientLight(0x404040, 1.5);
    scene.add(ambientLight);

    // Directional light for depth
    const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
    directionalLight.position.set(100, 100, 100);
    scene.add(directionalLight);

    // Point light following camera with primary blue
    const pointLight = new THREE.PointLight(0x3D85F5, 1, 800);
    pointLight.position.set(0, 0, 200);
    scene.add(pointLight);

    // Node click handler
    graph.onNodeClick(node => {
        // Zoom to node
        const distance = 150;
        const distRatio = 1 + distance / Math.hypot(node.x, node.y, node.z);

        graph.cameraPosition(
            { x: node.x * distRatio, y: node.y * distRatio, z: node.z * distRatio },
            node,
            3000
        );
    });

    console.log('3D Knowledge Graph initialized');
}

function getNodeSize(node) {
    // Size based on retrieval count and number of connections
    const baseSize = 4;
    const retrievalBonus = Math.min(node.retrievalCount || 0, 20) * 0.3;
    return baseSize + retrievalBonus;
}

function getNodeColor(node) {
    // Color based on retrieval count (cool to warm in design system palette)
    const count = node.retrievalCount || 0;

    if (count === 0) return 0x3D85F5;      // Primary blue - never accessed
    if (count < 3) return 0x5599FF;        // Light blue - rarely accessed
    if (count < 7) return 0x00B894;        // Success green - sometimes accessed
    if (count < 15) return 0xFF9500;       // Warning orange - frequently accessed
    return 0xFF3B30;                       // Error red - very frequently accessed
}

function setupControls() {
    // Center view button
    document.getElementById('centerView').addEventListener('click', () => {
        if (!graph || !graphData.nodes.length) return;

        // Reset camera to show all nodes
        graph.zoomToFit(1000, 50);
    });

    // Pause/Resume button
    const pauseBtn = document.getElementById('togglePause');
    pauseBtn.addEventListener('click', () => {
        isPaused = !isPaused;

        if (isPaused) {
            graph.pauseAnimation();
            pauseBtn.textContent = 'Resume';
            pauseBtn.style.background = '#00B894';
        } else {
            graph.resumeAnimation();
            pauseBtn.textContent = 'Pause';
            pauseBtn.style.background = '#3D85F5';
        }
    });
}

function updateStats(nodeCount, edgeCount) {
    document.getElementById('nodeCount').textContent = nodeCount;
    document.getElementById('edgeCount').textContent = edgeCount;
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Main function called by Swift with graph data
window.renderGraph = function(data) {
    console.log('Rendering graph with data:', data);

    if (!data || !data.nodes || !data.edges) {
        console.error('Invalid graph data received');
        return;
    }

    // Transform backend data to force-graph format
    graphData = {
        nodes: data.nodes.map(node => ({
            id: node.id,
            content: node.content || 'No content',
            context: node.context || '',
            keywords: node.keywords || [],
            tags: node.tags || [],
            category: node.category || null,
            retrievalCount: node.retrieval_count || 0,
            lastAccessed: node.last_accessed,
            createdAt: node.created_at
        })),
        links: data.edges.map(edge => ({
            source: edge.source,
            target: edge.target,
            strength: edge.strength || 0.5,
            rationale: edge.rationale || null
        }))
    };

    console.log(`Transformed data: ${graphData.nodes.length} nodes, ${graphData.links.length} links`);

    // Update the graph
    if (graph) {
        graph.graphData(graphData);

        // Update statistics
        updateStats(graphData.nodes.length, graphData.links.length);

        // Auto-zoom to fit after a moment
        setTimeout(() => {
            graph.zoomToFit(1000, 100);
        }, 500);

        console.log('Graph rendered successfully!');
    } else {
        console.error('Graph not initialized');
    }
};

// Handle window resize
window.addEventListener('resize', () => {
    if (graph) {
        graph.width(window.innerWidth);
        graph.height(window.innerHeight);
    }
});

// Export for debugging
window.getGraphData = () => graphData;
window.getGraph = () => graph;
