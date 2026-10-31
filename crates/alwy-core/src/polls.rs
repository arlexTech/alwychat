//! Polls (MSC3381 unstable events, as Element and FluffyChat send them).

use matrix_sdk::ruma::{
    OwnedEventId,
    events::poll::{
        start::PollKind,
        unstable_end::UnstablePollEndEventContent,
        unstable_response::UnstablePollResponseEventContent,
        unstable_start::{
            NewUnstablePollStartEventContent, UnstablePollAnswer, UnstablePollAnswers,
            UnstablePollStartContentBlock, UnstablePollStartEventContent,
        },
    },
};

use crate::{Error, Result};

impl crate::Core {
    /// Start a poll. `disclosed`: everyone sees the results while it runs.
    pub async fn create_poll(
        &self,
        room_id: &str,
        question: &str,
        answers: &[String],
        disclosed: bool,
        multiple: bool,
    ) -> Result<()> {
        let answers: Vec<UnstablePollAnswer> = answers
            .iter()
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
            .enumerate()
            .map(|(i, text)| UnstablePollAnswer::new(format!("a{i}"), text))
            .collect();
        let count = answers.len();
        let answers = UnstablePollAnswers::try_from(answers)
            .map_err(|_| Error::Poll("a poll needs 2 to 20 answers".into()))?;
        let mut block = UnstablePollStartContentBlock::new(question.trim(), answers);
        block.kind = if disclosed {
            PollKind::Disclosed
        } else {
            PollKind::Undisclosed
        };
        if multiple {
            block.max_selections = (count as u32).into();
        }
        let content =
            UnstablePollStartEventContent::New(NewUnstablePollStartEventContent::new(block));
        self.timeline(room_id)?
            .send(content.into())
            .await
            .map_err(|e| Error::Poll(e.to_string()))?;
        Ok(())
    }

    /// Vote (replaces our previous vote; an empty list withdraws it).
    pub async fn vote_poll(
        &self,
        room_id: &str,
        poll_id: &str,
        answers: Vec<String>,
    ) -> Result<()> {
        let poll_id = OwnedEventId::try_from(poll_id).map_err(|_| Error::UnknownEvent)?;
        self.timeline(room_id)?
            .send(UnstablePollResponseEventContent::new(answers, poll_id).into())
            .await
            .map_err(|e| Error::Poll(e.to_string()))?;
        Ok(())
    }

    /// Close a poll we started: no more votes, results shown to everyone.
    pub async fn end_poll(&self, room_id: &str, poll_id: &str) -> Result<()> {
        let poll_id = OwnedEventId::try_from(poll_id).map_err(|_| Error::UnknownEvent)?;
        self.timeline(room_id)?
            .send(UnstablePollEndEventContent::new("The poll has ended", poll_id).into())
            .await
            .map_err(|e| Error::Poll(e.to_string()))?;
        Ok(())
    }
}
