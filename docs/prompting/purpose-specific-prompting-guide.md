# Purpose-Specific Prompting Guide

## Table of Contents
- [Introduction](#introduction)
- [Information Retrieval and Question Answering](#information-retrieval-and-question-answering)
- [Creative Content Generation](#creative-content-generation)
- [Data Analysis and Problem Solving](#data-analysis-and-problem-solving)
- [Code Generation and Technical Tasks](#code-generation-and-technical-tasks)
- [Educational and Explanatory Content](#educational-and-explanatory-content)
- [Conversational Agents and Assistants](#conversational-agents-and-assistants)
- [Summary and Best Practices](#summary-and-best-practices)

## Introduction

This guide provides specialized prompting strategies for different task types when working with Large Language Models (LLMs). While the [Advanced LLM Prompting Techniques](advanced-llm-prompting-techniques.md) document covers general techniques, this guide focuses on tailoring your prompt approach to specific purposes and objectives.

## Information Retrieval and Question Answering

### Factual Questions

**Best Techniques**:
- Zero-shot prompting with clear, concise questions
- ReAct when external information verification is needed

**Example Prompt**:
```
I need accurate information about [topic]:
- What is [specific question]?
- When did [specific event] occur?
- Who was responsible for [specific accomplishment]?

Please cite your sources if you're drawing on specific publications or data.
```

### Research Questions

**Best Techniques**:
- Chain-of-Thought for connecting multiple facts
- ReAct for verification of complex information

**Example Prompt**:
```
I'm researching [topic] and need a comprehensive analysis:

1. What are the key developments in this field from [year] to present?
2. Who are the leading authorities and what are their main contributions?
3. What are the current debates or unresolved questions?
4. How has the understanding of [topic] evolved over time?

Please structure your response as a research brief with sections for each question.
```

### Information Synthesis

**Best Techniques**:
- Tree of Thoughts for exploring different analytical angles
- Self-reflection to ensure balanced representation

**Example Prompt**:
```
Synthesize information from multiple perspectives on [controversial topic]:

1. Summarize the main viewpoints (minimum 3 different perspectives)
2. Identify points of consensus and disagreement
3. Explain the evidence supporting each major position
4. Highlight any gaps in current knowledge

Ensure balanced treatment of different viewpoints without favoring any particular position.
```

## Creative Content Generation

### Storytelling

**Best Techniques**:
- Detailed context and character specifications
- Examples of desired style/tone
- Higher temperature settings (0.7-0.9)

**Example Prompt**:
```
Write a [genre] short story with these elements:
- Main character: [brief description]
- Setting: [time/place]
- Central conflict: [description]
- Theme: [theme]
- Tone: [e.g., humorous, dark, inspirational]

Style reference: Similar to [author/work], particularly in terms of [specific stylistic elements].

The story should be approximately [length] words and include dialogue, sensory details, and a satisfying conclusion.
```

### Marketing and Persuasive Content

**Best Techniques**:
- Clear target audience definition
- Specific goals and constraints
- Examples of tone and messaging

**Example Prompt**:
```
Create marketing copy for [product/service] targeting [specific audience]:

Product details:
- [key feature 1]
- [key feature 2]
- [key feature 3]

Unique selling proposition: [USP]
Customer pain points: [list of pain points]
Brand voice: [description of brand voice]

Create:
1. A compelling headline (maximum 10 words)
2. Three subheadlines highlighting key benefits
3. Body copy (150-200 words) that addresses pain points and emphasizes the USP
4. A clear call-to-action

The copy should emphasize [benefit/emotion] and avoid [tone/approach to avoid].
```

### Visual Descriptions and Design Prompts

**Best Techniques**:
- Highly detailed specifications
- Reference to existing styles/works
- Structured composition elements

**Example Prompt**:
```
Create a detailed description for [image type] featuring [subject]:

Setting/Background: [description]
Main elements: [list key visual elements]
Composition: [describe arrangement]
Lighting: [describe lighting conditions]
Color palette: [describe colors]
Style: [e.g., photorealistic, cartoon, impressionist]
Mood/Atmosphere: [describe intended feeling]

This description will be used to guide an AI image generator, so include specific details about textures, perspectives, and focal points.
```

## Data Analysis and Problem Solving

### Analytical Reasoning

**Best Techniques**:
- Chain-of-Thought for step-by-step analysis
- Tree of Thoughts for exploring multiple approaches
- Self-consistency for verification

**Example Prompt**:
```
Analyze the following [problem/scenario]:

[Detailed problem description with relevant data]

Please approach this analysis as follows:
1. Identify the key variables and constraints
2. Explore at least three different analytical approaches
3. For each approach:
   - Outline the methodology
   - Walk through the analysis step-by-step
   - Identify strengths and limitations
4. Recommend the most appropriate approach with justification
5. Provide your final analysis and conclusions

Show your reasoning at each step and explain how you arrived at your conclusions.
```

### Decision Support

**Best Techniques**:
- Tree of Thoughts for decision tree exploration
- Pros/cons analysis with weighted considerations
- Multiple expert perspectives

**Example Prompt**:
```
Help me make a decision about [decision context]:

Options under consideration:
- Option A: [description]
- Option B: [description]
- Option C: [description]

Key decision criteria:
- [Criterion 1] (importance: high/medium/low)
- [Criterion 2] (importance: high/medium/low)
- [Criterion 3] (importance: high/medium/low)

Additional context:
[Any relevant constraints, preferences, or background information]

Please provide:
1. A structured evaluation of each option against all criteria
2. Identification of potential risks and mitigations for each option
3. A clear recommendation with justification
4. 1-2 alternative recommendations to consider
```

### Data Interpretation

**Best Techniques**:
- Structured analysis frameworks
- Step-by-step reasoning
- Visual representation descriptions

**Example Prompt**:
```
Interpret the following data set:

[Data description or table]

Please provide:
1. Summary statistics and key observations
2. Identification of patterns, trends, and outliers
3. Possible explanations for the observed patterns
4. Limitations of the data and analysis
5. Recommendations for further analysis or data collection

If applicable, describe what visualizations would be most appropriate for this data and why.
```

## Code Generation and Technical Tasks

### Code Writing

**Best Techniques**:
- Detailed specification of requirements and constraints
- Examples of desired code style or approach
- ReAct for testing and debugging integration

**Example Prompt**:
```
Write [language] code to accomplish the following:

Functionality requirements:
- [Requirement 1]
- [Requirement 2]
- [Requirement 3]

Technical constraints:
- [Constraint 1, e.g., "Must work with version X"]
- [Constraint 2, e.g., "Memory usage restrictions"]

Expected inputs/outputs:
- Input: [example input format]
- Expected output: [example output format]

Code style preferences:
- [Style preference 1, e.g., "Use functional approach"]
- [Style preference 2, e.g., "Include detailed comments"]

Please include:
1. The complete code solution
2. Explanation of key components and design decisions
3. Instructions for running the code
4. Any edge cases or limitations to be aware of
```

### Debugging and Code Improvement

**Best Techniques**:
- Chain-of-Thought for tracing execution
- ReAct for testing hypotheses
- Problem decomposition

**Example Prompt**:
```
Help me debug/improve the following code:

```[language]
[code block]
```

Current behavior: [description of current behavior]
Expected behavior: [description of expected behavior]

Error messages (if any):
[error messages]

Please:
1. Identify potential issues in the code
2. Explain why each issue might be causing the observed behavior
3. Provide fixes for each issue
4. Suggest any performance improvements or best practices that could be applied
5. Provide the improved version of the code

Walk through your debugging process step by step, explaining your reasoning.
```

### API and Technology Integration

**Best Techniques**:
- Clear specification of systems to integrate
- Examples of expected data flows
- ReAct for verification of approach

**Example Prompt**:
```
I need to integrate [Technology A] with [Technology B]:

Details about Technology A:
- [API endpoints/relevant features]
- [Authentication method]
- [Data format]

Details about Technology B:
- [API endpoints/relevant features]
- [Authentication method]
- [Data format]

Integration requirements:
- [Requirement 1]
- [Requirement 2]

Provide a detailed integration plan including:
1. Architecture diagram description
2. Key code snippets for critical integration points
3. Data transformation requirements
4. Error handling approach
5. Testing strategy
```

## Educational and Explanatory Content

### Concept Explanation

**Best Techniques**:
- Audience-appropriate language and examples
- Multi-level explanations (simple to complex)
- Analogies and visualizations

**Example Prompt**:
```
Explain [complex concept] to [target audience, e.g., "a high school student", "someone with no technical background"]:

Please structure your explanation as follows:
1. Simple one-paragraph overview using everyday language
2. Key components or principles (3-5)
3. Real-world examples or analogies that illustrate the concept
4. Common misconceptions and clarifications
5. Practical applications or relevance
6. More technical explanation for those who want to go deeper

Use analogies, visual descriptions, and concrete examples throughout. Avoid jargon unless absolutely necessary (and explain it when used).
```

### Step-by-Step Tutorials

**Best Techniques**:
- Clear prerequisites and sequential steps
- Anticipation of common errors
- Checkpoints for verification

**Example Prompt**:
```
Create a tutorial for [specific task/skill]:

Prerequisites:
- [Prerequisite 1]
- [Prerequisite 2]

Materials/tools needed:
- [Item 1]
- [Item 2]

Please structure the tutorial as follows:
1. Introduction and end goal (with example of finished result)
2. Preparation steps
3. Main procedure broken down into clear, numbered steps
   - Include estimated time for each major section
   - Note potential difficulties and how to overcome them
   - Include checkpoints to verify correct progression
4. Troubleshooting section for common issues
5. Extensions or variations

For each step, explain both WHAT to do and WHY it's done that way. Include safety precautions or best practices where relevant.
```

### Comparative Explanations

**Best Techniques**:
- Structured comparison frameworks
- Balance between similarities and differences
- Multiple evaluation criteria

**Example Prompt**:
```
Compare and contrast [Topic A] and [Topic B] for [specific audience/purpose]:

Please include:
1. Brief introduction to both topics
2. Comparison table covering:
   - [Criterion 1]
   - [Criterion 2]
   - [Criterion 3]
   - [Criterion 4]
3. Key similarities (minimum 3)
4. Significant differences (minimum 3)
5. Scenarios where one might be preferred over the other
6. Current trends or developments affecting both

Ensure the comparison is balanced, objective, and provides meaningful insights rather than surface-level differences.
```

## Conversational Agents and Assistants

### Persona Development

**Best Techniques**:
- Detailed character specifications
- Contextual background information
- Consistent language patterns

**Example Prompt**:
```
I'm developing a conversational agent with the following persona:

Character profile:
- Name: [Name]
- Background: [Brief history/context]
- Expertise: [Areas of knowledge]
- Personality traits: [List 3-5 key traits]
- Communication style: [Description of language patterns]
- Values: [Core values that guide responses]

Response parameters:
- Formality level: [formal/semi-formal/casual]
- Typical response length: [brief/moderate/detailed]
- Use of humor: [frequency and style]
- Approach to uncertainty: [how to handle questions without clear answers]

Interaction goals:
- [Primary purpose of the agent]
- [Secondary purposes]

Please create 5 example exchanges that demonstrate this persona responding to different types of user inputs, including:
1. A straightforward information request
2. An ambiguous question
3. A request outside the agent's expertise
4. An emotionally charged situation
5. A follow-up clarification
```

### Conversation Design

**Best Techniques**:
- Clear conversational flows
- Anticipation of user intents
- Fallback and recovery strategies

**Example Prompt**:
```
Design a conversation flow for a [type of assistant] helping users with [specific task]:

User context: [Relevant user background information]
Conversation goal: [Desired outcome]

Please create a complete conversation flow including:
1. Initial greeting and context setting
2. Key questions the assistant should ask to gather necessary information
3. Potential user responses and appropriate follow-ups for each (at least 2 variations)
4. How to handle expected user questions or concerns
5. Error recovery paths when:
   - The user provides incomplete information
   - The user requests something outside the scope
   - The conversation gets off track
6. Successful resolution paths and appropriate closing

For each assistant response, include:
- The actual response text
- Notes on the purpose of that response
- Any conditional logic (if applicable)
```

### Multi-turn Interaction Design

**Best Techniques**:
- Memory utilization and context tracking
- Progressive information gathering
- Contextual response adaptation

**Example Prompt**:
```
Design a multi-turn interaction strategy for a [type of agent] that needs to [accomplish specific goal]:

Interaction requirements:
- Information needed from user: [list of data points to collect]
- Constraints: [any limitations to work within]
- User experience priorities: [e.g., efficiency, empathy, clarity]

Please create:
1. A strategy for maintaining context across multiple turns
2. Techniques for elegantly gathering required information without overwhelming the user
3. Methods for confirming understanding and correcting misunderstandings
4. Approaches for adapting to changing user needs mid-conversation
5. A framework for progressive disclosure of complex information

Include example dialogue snippets demonstrating each of these elements, showing both user and system turns.
```

## Summary and Best Practices

### Cross-Purpose Recommendations

1. **Start with the end in mind**: Define your desired outcome before crafting your prompt
2. **Know your audience**: Adjust complexity and style to match the intended user
3. **Be specific and detailed**: Provide clear constraints and examples
4. **Iterate and refine**: Test prompts with variations to optimize results
5. **Use structural elements**: Organize prompts with clear sections and formatting
6. **Include examples**: Demonstrate desired outputs when possible
7. **Balance constraints with flexibility**: Guide the model without overly restricting creative solutions

### Testing Your Prompts

When implementing purpose-specific prompting:

1. **Evaluate with edge cases**: Test with unusual or boundary inputs
2. **Gather user feedback**: Monitor how real users respond to outputs
3. **Compare variations**: Test slight modifications to identify optimal approaches
4. **Use A/B testing**: Systematically compare alternative prompting strategies
5. **Document what works**: Keep a library of effective prompts for different purposes

### Continuous Improvement

The field of prompt engineering is rapidly evolving. Stay current by:

1. Following developments in LLM capabilities
2. Experimenting with new prompting techniques
3. Building upon successful patterns
4. Sharing knowledge with the prompt engineering community
5. Adapting strategies as models and best practices evolve

---

This guide complements the [Advanced LLM Prompting Techniques](advanced-llm-prompting-techniques.md) document by focusing on purpose-specific applications of prompting strategies. For information on the underlying techniques mentioned (Chain-of-Thought, ReAct, Tree of Thoughts, etc.), please refer to the primary document.